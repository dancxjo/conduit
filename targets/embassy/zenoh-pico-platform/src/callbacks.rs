//! Checked extents and typed worker operations at the foreign-code boundary.
//! Raw memory validity is still the C caller's safety obligation.
use crate::{BridgeAccess, CArena, SocketWorker, WorkerRuntime};
use conduit_zenoh_pico_platform::{Endpoint, Fault};
use core::{
    fmt::{self, Write},
    slice,
};

/// Real target resources, not fallback services. All calls run on the isolated
/// C worker. Endpoint snapshots must be synchronized with the actual owner.
pub trait CallbackPlatform {
    fn boot_us(&self) -> u64;
    /// Refuse durations outside the admitted worker/timer bounds.
    fn sleep_us(&self, duration: u64) -> Result<(), Fault>;
    fn random_fill(&mut self, output: &mut [u8]) -> Result<(), Fault>;
    /// Real Unix time with provenance, or None. Never return boot time here.
    fn wall_time(&self) -> Option<(u32, u32)>;
    fn endpoints(&self) -> Option<(u32, Endpoint, Endpoint)>;
    /// Retire/fail-stop this worker when the void RNG ABI cannot report error;
    /// never substitute zero bytes. Target policy preserves other local work.
    fn entropy_failure(&self) -> !;
}
/// Single-worker context. It creates no global callbacks, thread or admission.
pub struct Callbacks<'a, A, R, P, const RX: usize, const TX: usize, const ALIGN: usize> {
    worker: SocketWorker<'a, A, R, RX, TX>,
    arena: CArena<ALIGN>,
    platform: P,
}
impl<
        'a,
        A: BridgeAccess<RX, TX>,
        R: WorkerRuntime,
        P: CallbackPlatform,
        const RX: usize,
        const TX: usize,
        const ALIGN: usize,
    > Callbacks<'a, A, R, P, RX, TX, ALIGN>
{
    pub fn new(worker: SocketWorker<'a, A, R, RX, TX>, arena: CArena<ALIGN>, platform: P) -> Self {
        Self {
            worker,
            arena,
            platform,
        }
    }
    pub fn arena_stats(&self) -> crate::ArenaStats {
        self.arena.stats()
    }
    pub fn allocate(&mut self, size: usize) -> *mut u8 {
        self.arena.allocate(size)
    }
    /// # Safety
    /// Pointer satisfies CArena::free's live allocation requirements.
    pub unsafe fn free(&mut self, pointer: *mut u8) {
        unsafe {
            self.arena.free(pointer);
        }
    }
    /// # Safety
    /// Pointer satisfies CArena::reallocate's live allocation requirements.
    pub unsafe fn reallocate(&mut self, pointer: *mut u8, size: usize) -> *mut u8 {
        unsafe { self.arena.reallocate(pointer, size) }
    }
    pub fn boot_us(&self) -> u64 {
        self.platform.boot_us()
    }
    pub fn sleep_us(&self, duration: u64) -> i32 {
        if self.platform.sleep_us(duration).is_ok() {
            0
        } else {
            -1
        }
    }
    /// # Safety
    /// Nonempty output is a valid writable extent, disjoint from this context.
    pub unsafe fn random_fill(&mut self, output: *mut u8, len: usize) {
        if len == 0 {
            return;
        }
        if output.is_null() || len > isize::MAX as usize {
            self.platform.entropy_failure();
        }
        // SAFETY: caller owns the valid output extent; no reference is retained.
        let output = unsafe { slice::from_raw_parts_mut(output, len) };
        if self.platform.random_fill(output).is_err() {
            self.platform.entropy_failure();
        }
    }
    /// # Safety
    /// Nonnull outputs are valid aligned writable u32 pointers.
    pub unsafe fn wall_time(&self, secs: *mut u32, nanos: *mut u32) -> i32 {
        if secs.is_null() || nanos.is_null() || secs == nanos {
            return -1;
        }
        let Some((s, n)) = self.platform.wall_time() else {
            return -1;
        };
        if n >= 1_000_000_000 {
            return -1;
        }
        // SAFETY: caller supplies valid outputs. No timestamp is invented.
        unsafe {
            secs.write(s);
            nanos.write(n);
        }
        0
    }
    /// # Safety
    /// IPv4 has four readable bytes; nonnull outputs are valid writable u32s.
    pub unsafe fn connect(
        &mut self,
        ipv4: *const u8,
        port: u16,
        timeout: u32,
        slot: *mut u32,
        generation: *mut u32,
    ) -> i32 {
        if slot.is_null() || generation.is_null() || slot == generation {
            return -1;
        }
        // Snapshot before writing outputs, since C does not declare restrict.
        let address = if ipv4.is_null() {
            None
        } else {
            Some(unsafe { ipv4.cast::<[u8; 4]>().read() })
        };
        // SAFETY: caller supplies the exact input and output extents.
        unsafe {
            slot.write(0);
            generation.write(0);
        }
        let Some(address) = address else {
            return -1;
        };
        match self.worker.connect(
            Endpoint {
                ipv4: address,
                port,
            },
            timeout,
        ) {
            Ok(g) => {
                unsafe {
                    generation.write(g);
                }
                0
            }
            Err(_) => -1,
        }
    }
    /// # Safety
    /// Nonempty output is a valid writable extent, disjoint from this context.
    pub unsafe fn read(
        &mut self,
        slot: u32,
        generation: u32,
        output: *mut u8,
        len: usize,
        timeout: u32,
    ) -> isize {
        if slot != 0 || len > isize::MAX as usize || (len != 0 && output.is_null()) {
            return -1;
        }
        let output = if len == 0 {
            &mut []
        } else {
            // SAFETY: caller owns this extent; it is never retained after return.
            unsafe { slice::from_raw_parts_mut(output, len) }
        };
        self.worker
            .read(generation, output, timeout)
            .map_or(-1, |n| n as isize)
    }
    /// # Safety
    /// Nonempty input is a valid readable extent, disjoint from this context.
    pub unsafe fn write(
        &mut self,
        slot: u32,
        generation: u32,
        input: *const u8,
        len: usize,
        timeout: u32,
    ) -> isize {
        if slot != 0 || len > isize::MAX as usize || (len != 0 && input.is_null()) {
            return -1;
        }
        let input = if len == 0 {
            &[]
        } else {
            // SAFETY: caller owns this extent; it is copied synchronously only.
            unsafe { slice::from_raw_parts(input, len) }
        };
        self.worker
            .write(generation, input, timeout)
            .map_or(-1, |n| n as isize)
    }
    pub fn close(&mut self, slot: u32, generation: u32) -> i32 {
        if slot != 0 {
            return -1;
        }
        self.worker.close(generation).map_or(-1, |_| 0)
    }
    pub fn ready(&mut self, slot: u32, generation: u32, timeout: u32) -> i32 {
        if slot != 0 {
            return -1;
        }
        self.worker.ready(generation, timeout).map_or(-1, i32::from)
    }
    pub fn set_blocking(&mut self, slot: u32, generation: u32, blocking: bool) -> i32 {
        if slot != 0 {
            return -1;
        }
        self.worker
            .set_blocking(generation, blocking)
            .map_or(-1, |_| 0)
    }
    /// # Safety
    /// Each nonnull output has its declared writable extent. The outputs do not
    /// overlap this context; they are never retained after return.
    pub unsafe fn endpoints(
        &self,
        slot: u32,
        generation: u32,
        local: *mut u8,
        local_len: usize,
        remote: *mut u8,
        remote_len: usize,
    ) -> i32 {
        if slot != 0
            || local.is_null()
            || remote.is_null()
            || self.worker.generation(generation).is_err()
        {
            return -1;
        }
        let Some((g, l, r)) = self.platform.endpoints() else {
            return -1;
        };
        if g != generation {
            return -1;
        }
        let (l, r) = (Text::endpoint(l), Text::endpoint(r));
        if local_len <= l.len || remote_len <= r.len {
            return -1;
        }
        // SAFETY: caller owns both output extents, including the NUL byte.
        unsafe {
            l.copy_to(local);
            r.copy_to(remote);
        }
        0
    }
}
struct Text {
    bytes: [u8; 22],
    len: usize,
}
impl Text {
    fn endpoint(e: Endpoint) -> Self {
        let mut t = Self {
            bytes: [0; 22],
            len: 0,
        };
        let [a, b, c, d] = e.ipv4;
        // Maximum IPv4:port is 21 bytes, with one byte retained for NUL.
        write!(&mut t, "{a}.{b}.{c}.{d}:{}", e.port).expect("IPv4 endpoint bound");
        t
    }
    unsafe fn copy_to(&self, output: *mut u8) {
        unsafe {
            core::ptr::copy_nonoverlapping(self.bytes.as_ptr(), output, self.len + 1);
        }
    }
}
impl Write for Text {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self
            .len
            .checked_add(s.len())
            .filter(|end| *end < self.bytes.len())
            .ok_or(fmt::Error)?;
        self.bytes[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}
