//! Finite cryptographic input supplied fresh by the reviewed emulator host.
//! This is a virtual-platform provider, not a LoongArch hardware RNG claim.
//! Its MMIO reader uses no DMA; all retained bytes remain privileged Root state.
use crate::cryptographic_entropy::{CryptographicEntropySource, EntropyProvider, EntropyRefusal};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};
#[path = "entropy_directory.rs"]
mod directory;
const DATA: *const u8 = 0x1e02_0000 as *const u8;
const SELECT: *mut u16 = 0x1e02_0008 as *mut u16;
const DIRECTORY_LIMIT: u32 = 256;

struct State {
    bytes: [u8; directory::INPUT_BYTES],
    used: usize,
    initialized: bool,
    failed: bool,
}
struct Pool(UnsafeCell<State>);
// The guard owns every access to this fixed Boot-scoped pool.
unsafe impl Sync for Pool {}
static POOL: Pool = Pool(UnsafeCell::new(State {
    bytes: [0; directory::INPUT_BYTES],
    used: 0,
    initialized: false,
    failed: false,
}));
static LOCK: AtomicBool = AtomicBool::new(false);
struct Guard(usize);
impl Guard {
    fn acquire() -> Result<Self, EntropyRefusal> {
        let status = super::read_csr::<0>();
        super::disable_interrupts();
        if LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            if status & 4 != 0 {
                super::enable_interrupts();
            }
            return Err(EntropyRefusal::SourceFailure);
        }
        Ok(Self(status))
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        LOCK.store(false, Ordering::Release);
        if self.0 & 4 != 0 {
            super::enable_interrupts();
        }
    }
}
pub struct FwCfgEntropy;
impl FwCfgEntropy {
    pub fn detect(generation: u64) -> Result<Self, EntropyRefusal> {
        if generation == 0 {
            return Err(EntropyRefusal::InvalidProvider);
        }
        if generation != 1 {
            return Err(EntropyRefusal::StaleProvider);
        }
        let _guard = Guard::acquire()?;
        let state = unsafe { &mut *POOL.0.get() };
        if state.failed {
            return Err(EntropyRefusal::Unavailable);
        }
        if !state.initialized {
            if let Err(error) = load(&mut state.bytes) {
                erase(&mut state.bytes);
                state.failed = true;
                return Err(error);
            }
            state.initialized = true;
        }
        Ok(Self)
    }
}
impl CryptographicEntropySource for FwCfgEntropy {
    fn provider(&self) -> EntropyProvider {
        EntropyProvider {
            base_id: "conduitos/base/cryptographic-entropy/fwcfg-linux-getrandom@1",
            provider_instance_id: "conduitos/loongarch64/virt/fwcfg-entropy@1",
            provider_generation: 1,
        }
    }
    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        erase(output);
        if output.is_empty() {
            return Err(EntropyRefusal::EmptyBuffer);
        }
        if output.len() > 64 {
            return Err(EntropyRefusal::BufferTooLarge);
        }
        let _guard = Guard::acquire()?;
        let state = unsafe { &mut *POOL.0.get() };
        if !state.initialized || state.failed {
            return Err(EntropyRefusal::Unavailable);
        }
        let end = state
            .used
            .checked_add(output.len())
            .filter(|end| *end <= state.bytes.len())
            .ok_or(EntropyRefusal::RequestCapacity)?;
        output.copy_from_slice(&state.bytes[state.used..end]);
        erase(&mut state.bytes[state.used..end]);
        state.used = end;
        Ok(())
    }
}
fn erase(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}

#[cfg(feature = "ordinary-domain-proof")]
pub fn consumed_storage_erased() -> bool {
    let Ok(_guard) = Guard::acquire() else {
        return false;
    };
    let state = unsafe { &*POOL.0.get() };
    state.bytes[..state.used].iter().all(|byte| *byte == 0)
}
#[cfg(feature = "ordinary-domain-proof")]
pub fn storage_address() -> u64 {
    core::ptr::addr_of!(POOL) as u64
}
fn select(index: u16) {
    unsafe {
        SELECT.write_volatile(index.to_be());
        core::arch::asm!("dbar 0", options(nostack));
    }
}
fn read(bytes: &mut [u8]) {
    for byte in bytes {
        *byte = unsafe { DATA.read_volatile() };
    }
}
fn load(output: &mut [u8; directory::INPUT_BYTES]) -> Result<(), EntropyRefusal> {
    select(0);
    let mut signature = [0; 4];
    read(&mut signature);
    if &signature != b"QEMU" {
        return Err(EntropyRefusal::Unavailable);
    }
    select(0x19);
    let mut count = [0; 4];
    read(&mut count);
    let count = u32::from_be_bytes(count);
    if count > DIRECTORY_LIMIT {
        return Err(EntropyRefusal::SourceFailure);
    }
    let mut found = None;
    for _ in 0..count {
        let mut entry = [0; 64];
        read(&mut entry);
        if let Some(selector) = directory::selector(&entry)? {
            if found.replace(selector).is_some() {
                return Err(EntropyRefusal::SourceFailure);
            }
        }
    }
    select(found.ok_or(EntropyRefusal::Unavailable)?);
    read(output);
    if output.iter().all(|byte| *byte == 0) {
        return Err(EntropyRefusal::WeakOutput);
    }
    Ok(())
}
