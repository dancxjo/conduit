//! Exported ABI against explicit test-only resources; no network/device evidence.
use conduit_zenoh_pico_embassy::{
    export_zenoh_pico_callbacks, BridgeAccess, CArena, CallbackPlatform, Callbacks, SocketWorker,
    WorkerRuntime,
};
use conduit_zenoh_pico_platform::{Endpoint, Fault, NetworkCommand, SocketBridge};
use std::{
    cell::{Cell, RefCell},
    mem::MaybeUninit,
    ptr,
};
const PEER: Endpoint = Endpoint {
    ipv4: [192, 0, 2, 2],
    port: 7447,
};
struct Access(RefCell<SocketBridge<4, 4>>);
impl BridgeAccess<4, 4> for Access {
    fn with<T>(&self, f: impl FnOnce(&mut SocketBridge<4, 4>) -> T) -> T {
        f(&mut self.0.borrow_mut())
    }
}
struct Control {
    now: Cell<u64>,
    wall: Cell<Option<(u32, u32)>>,
    endpoints: Cell<Option<(u32, Endpoint, Endpoint)>>,
    slept: Cell<u64>,
}
struct Runtime {
    access: &'static Access,
    control: &'static Control,
}
impl WorkerRuntime for Runtime {
    fn now_ms(&self) -> u64 {
        self.control.now.get()
    }
    fn yield_worker(&self) -> Result<(), Fault> {
        self.control.now.set(self.control.now.get() + 1);
        self.access
            .with(|b| match b.take_command(self.control.now.get()) {
                Some(NetworkCommand::Connect { generation, .. }) => {
                    b.complete_connect(generation, self.control.now.get(), Ok(()))
                        .unwrap();
                }
                Some(NetworkCommand::Close { generation }) => {
                    b.complete_close(generation).unwrap();
                }
                None => (),
            });
        Ok(())
    }
}
struct Platform(&'static Control);
impl CallbackPlatform for Platform {
    fn boot_us(&self) -> u64 {
        self.0.now.get() * 1000
    }
    fn sleep_us(&self, n: u64) -> Result<(), Fault> {
        if n > 20 {
            return Err(Fault::TimedOut);
        }
        self.0.slept.set(n);
        Ok(())
    }
    fn random_fill(&mut self, out: &mut [u8]) -> Result<(), Fault> {
        for (i, b) in out.iter_mut().enumerate() {
            *b = [0, 255, 128, 17][i % 4];
        }
        Ok(())
    }
    fn wall_time(&self) -> Option<(u32, u32)> {
        self.0.wall.get()
    }
    fn endpoints(&self) -> Option<(u32, Endpoint, Endpoint)> {
        self.0.endpoints.get()
    }
    fn entropy_failure(&self) -> ! {
        panic!("test does not exercise fail-stop through extern C")
    }
}
type Context = Callbacks<'static, Access, Runtime, Platform, 4, 4, 16>;
thread_local! {static CONTEXT:RefCell<Option<Context>>=const {RefCell::new(None)};}
fn with_context<T>(f: impl FnOnce(&mut Context) -> T) -> T {
    CONTEXT.with(|c| f(c.borrow_mut().as_mut().unwrap()))
}
export_zenoh_pico_callbacks!(with_context);
fn install() -> (&'static Access, &'static Control) {
    let a = Box::leak(Box::new(Access(RefCell::new(SocketBridge::new()))));
    let c = Box::leak(Box::new(Control {
        now: Cell::new(0),
        wall: Cell::new(None),
        endpoints: Cell::new(None),
        slept: Cell::new(0),
    }));
    let storage = Box::leak(vec![MaybeUninit::uninit(); 4096].into_boxed_slice());
    let arena = CArena::new(storage).unwrap();
    let worker = SocketWorker::new(
        a,
        Runtime {
            access: a,
            control: c,
        },
        3,
    )
    .unwrap();
    CONTEXT
        .with(|context| *context.borrow_mut() = Some(Callbacks::new(worker, arena, Platform(c))));
    (a, c)
}
fn connect() -> u32 {
    let mut slot = 99;
    let mut generation = 99;
    // SAFETY: IP and distinct aligned outputs have their exact declared extents.
    assert_eq!(
        unsafe {
            conduit_z_tcp_connect(
                PEER.ipv4.as_ptr(),
                PEER.port,
                10,
                &mut slot,
                &mut generation,
            )
        },
        0
    );
    assert_eq!(slot, 0);
    assert_ne!(generation, 0);
    generation
}
#[test]
fn exported_signatures_match_the_c_callback_abi() {
    let _: unsafe extern "C" fn(usize) -> *mut u8 = conduit_z_alloc;
    let _: unsafe extern "C" fn(*mut u8, usize) -> *mut u8 = conduit_z_realloc;
    let _: unsafe extern "C" fn(*mut u8) = conduit_z_free;
    let _: unsafe extern "C" fn() -> u64 = conduit_z_boot_us;
    let _: unsafe extern "C" fn(u64) -> i32 = conduit_z_sleep_us;
    let _: unsafe extern "C" fn(*mut u8, usize) = conduit_z_random_fill;
    let _: unsafe extern "C" fn(*mut u32, *mut u32) -> i32 = conduit_z_wall_time;
    let _: unsafe extern "C" fn(*const u8, u16, u32, *mut u32, *mut u32) -> i32 =
        conduit_z_tcp_connect;
    let _: unsafe extern "C" fn(u32, u32, *mut u8, usize, u32) -> isize = conduit_z_tcp_read;
    let _: unsafe extern "C" fn(u32, u32, *const u8, usize, u32) -> isize = conduit_z_tcp_write;
    let _: unsafe extern "C" fn(u32, u32) -> i32 = conduit_z_tcp_close;
    let _: unsafe extern "C" fn(u32, u32, u32) -> i32 = conduit_z_tcp_ready;
    let _: unsafe extern "C" fn(u32, u32, bool) -> i32 = conduit_z_tcp_set_blocking;
    let _: unsafe extern "C" fn(u32, u32, *mut u8, usize, *mut u8, usize) -> i32 =
        conduit_z_tcp_endpoints;
}
#[test]
fn checked_rejections_precede_raw_extent_creation_and_failed_connect_clears_outputs() {
    install();
    let generation = connect();
    let mut one = [42u8];
    // SAFETY: null/oversized/invalid-slot paths reject before constructing a raw
    // slice or dereferencing the supplied pointer; one is otherwise valid memory.
    unsafe {
        assert_eq!(conduit_z_tcp_read(0, generation, ptr::null_mut(), 1, 0), -1);
        assert_eq!(conduit_z_tcp_write(0, generation, ptr::null(), 1, 0), -1);
        assert_eq!(
            conduit_z_tcp_read(0, generation, one.as_mut_ptr(), usize::MAX, 0),
            -1
        );
        assert_eq!(
            conduit_z_tcp_write(0, generation, one.as_ptr(), usize::MAX, 0),
            -1
        );
        assert_eq!(
            conduit_z_tcp_read(1, generation, ptr::null_mut(), 10, 0),
            -1
        );
        assert_eq!(conduit_z_tcp_write(1, generation, ptr::null(), 10, 0), -1);
        assert_eq!(conduit_z_tcp_ready(1, generation, 0), -1);
        assert_eq!(conduit_z_tcp_close(1, generation), -1);
        assert_eq!(conduit_z_tcp_set_blocking(1, generation, false), -1);
        assert_eq!(conduit_z_tcp_read(0, generation, ptr::null_mut(), 0, 0), 0);
        assert_eq!(conduit_z_tcp_write(0, generation, ptr::null(), 0, 0), 0);
        let mut slot = 88;
        let mut g = 99;
        assert_eq!(
            conduit_z_tcp_connect(ptr::null(), 7447, 10, &mut slot, &mut g),
            -1
        );
        assert_eq!((slot, g), (0, 0));
        slot = 88;
        g = 99;
        assert_eq!(
            conduit_z_tcp_connect(PEER.ipv4.as_ptr(), 0, 10, &mut slot, &mut g),
            -1
        );
        assert_eq!((slot, g), (0, 0));
        let mut aliased: u32 = 77;
        let p = &mut aliased as *mut u32;
        assert_eq!(
            conduit_z_tcp_connect(PEER.ipv4.as_ptr(), 7447, 10, p, p),
            -1
        );
        assert_eq!(aliased, 77);
        assert_eq!(conduit_z_tcp_close(0, generation), 0);
    }
    assert_eq!(one, [42]);
}
#[test]
fn c_write_budget_survives_nonblocking_read_mode() {
    let (_, control) = install();
    let g = connect();
    let bytes = [0, 255, 128, 9];
    let mut out = [42];
    // SAFETY: both arrays own their declared extents throughout each callback.
    unsafe {
        assert_eq!(conduit_z_tcp_set_blocking(0, g, false), 0);
        assert_eq!(conduit_z_tcp_write(0, g, bytes.as_ptr(), 4, 3), 4);
        let before = control.now.get();
        assert_eq!(conduit_z_tcp_write(0, g, bytes.as_ptr(), 1, 3), -1);
        assert_eq!(control.now.get(), before + 3);
        assert_eq!(conduit_z_tcp_read(0, g, out.as_mut_ptr(), 1, 3), -1);
        assert_eq!(control.now.get(), before + 3);
        assert_eq!(out, [42]);
        assert_eq!(conduit_z_tcp_close(0, g), 0);
    }
}
#[test]
fn partial_binary_callbacks_preserve_prefix_and_read_eof_and_deadline() {
    let (access, _) = install();
    let g = connect();
    let input = [0, 255, 128, 9, 8, 7];
    let mut output = [42; 6];
    // SAFETY: input/output are live disjoint arrays of the declared sizes.
    unsafe {
        assert_eq!(
            conduit_z_tcp_write(0, g, input.as_ptr(), input.len(), 10),
            4
        );
        assert_eq!(conduit_z_tcp_write(0, g, input[4..].as_ptr(), 2, 0), -1);
        assert_eq!(conduit_z_tcp_ready(0, g, 0), 0);
        assert_eq!(
            conduit_z_tcp_read(0, g, output.as_mut_ptr(), output.len(), 2),
            -1
        );
        assert_eq!(output, [42; 6]);
    }
    access.with(|b| {
        let generation = b.generation().unwrap();
        let mut sent = [0; 4];
        let ticket = b.network_send(generation, &mut sent).unwrap().unwrap();
        assert_eq!(sent, [0, 255, 128, 9]);
        b.network_sent(ticket, 4).unwrap();
        assert_eq!(b.network_receive(generation, &[0, 255, 128]), Ok(3));
        b.peer_closed(generation).unwrap();
    });
    // SAFETY: output owns six writable bytes and callbacks do not retain it.
    unsafe {
        assert_eq!(conduit_z_tcp_ready(0, g, 0), 1);
        assert_eq!(conduit_z_tcp_read(0, g, output.as_mut_ptr(), 2, 10), 2);
        assert_eq!(&output[..2], &[0, 255]);
        assert_eq!(conduit_z_tcp_read(0, g, output.as_mut_ptr(), 6, 10), 1);
        assert_eq!(output[0], 128);
        assert_eq!(conduit_z_tcp_read(0, g, output.as_mut_ptr(), 6, 10), 0);
        assert_eq!(conduit_z_tcp_close(0, g), 0);
        assert_eq!(conduit_z_tcp_read(0, g, output.as_mut_ptr(), 6, 0), -1);
    }
}
#[test]
fn exported_arena_realloc_preserves_binary_and_finishes_with_zero_live_bytes() {
    install();
    let binary = [0, 255, 128, 7, 0, 19];
    // SAFETY: all nonnull frees/reallocations use this context's sole live allocation.
    unsafe {
        assert!(conduit_z_alloc(0).is_null());
        conduit_z_free(ptr::null_mut());
        let p = conduit_z_realloc(ptr::null_mut(), binary.len());
        assert!(!p.is_null());
        ptr::copy_nonoverlapping(binary.as_ptr(), p, binary.len());
        let p = conduit_z_realloc(p, 128);
        assert!(!p.is_null());
        assert_eq!(std::slice::from_raw_parts(p, binary.len()), binary);
        assert!(conduit_z_realloc(p, usize::MAX).is_null());
        assert_eq!(std::slice::from_raw_parts(p, binary.len()), binary);
        conduit_z_free(p);
    }
    with_context(|c| {
        let s = c.arena_stats();
        assert_eq!(s.used, 0);
        assert_eq!(s.requested, 0);
        assert_eq!(s.allocations, 0);
    });
}
#[test]
fn explicit_fixture_clock_entropy_sleep_and_wall_time_never_invent_missing_time() {
    let (_, control) = install();
    let mut bytes = [42; 13];
    let mut secs = 11;
    let mut nanos = 22;
    // SAFETY: fixture outputs are valid distinct arrays/u32s; null+zero RNG is a no-op.
    unsafe {
        assert_eq!(conduit_z_boot_us(), 0);
        assert_eq!(conduit_z_wall_time(&mut secs, &mut nanos), -1);
        assert_eq!((secs, nanos), (11, 22));
        control.wall.set(Some((1700000000, 1_000_000_000)));
        assert_eq!(conduit_z_wall_time(&mut secs, &mut nanos), -1);
        assert_eq!((secs, nanos), (11, 22));
        control.wall.set(Some((1700000000, 123)));
        assert_eq!(conduit_z_wall_time(&mut secs, &mut nanos), 0);
        assert_eq!((secs, nanos), (1700000000, 123));
        assert_eq!(conduit_z_wall_time(ptr::null_mut(), &mut nanos), -1);
        let mut aliased: u32 = 77;
        let same = &mut aliased as *mut u32;
        assert_eq!(conduit_z_wall_time(same, same), -1);
        assert_eq!(aliased, 77);
        conduit_z_random_fill(bytes.as_mut_ptr(), bytes.len());
        conduit_z_random_fill(ptr::null_mut(), 0);
        assert_eq!(bytes, std::array::from_fn(|i| [0, 255, 128, 17][i % 4]));
        assert_eq!(conduit_z_sleep_us(17), 0);
        assert_eq!(control.slept.get(), 17);
        assert_eq!(conduit_z_sleep_us(21), -1);
        assert_eq!(control.slept.get(), 17);
    }
}
#[test]
fn endpoint_snapshot_requires_current_generation_and_both_output_capacities() {
    let (_, control) = install();
    let g = connect();
    let mut local = [42; 22];
    let mut remote = [43; 22];
    let max = Endpoint {
        ipv4: [255; 4],
        port: 65535,
    };
    // SAFETY: both outputs own22 bytes, and refusal cases write neither buffer.
    unsafe {
        assert_eq!(
            conduit_z_tcp_endpoints(0, g, local.as_mut_ptr(), 22, remote.as_mut_ptr(), 22),
            -1
        );
        control.endpoints.set(Some((g + 1, max, max)));
        assert_eq!(
            conduit_z_tcp_endpoints(0, g, local.as_mut_ptr(), 22, remote.as_mut_ptr(), 22),
            -1
        );
        control.endpoints.set(Some((g, max, max)));
        assert_eq!(
            conduit_z_tcp_endpoints(0, g, local.as_mut_ptr(), 21, remote.as_mut_ptr(), 22),
            -1
        );
        assert_eq!(
            conduit_z_tcp_endpoints(0, g, local.as_mut_ptr(), 22, remote.as_mut_ptr(), 21),
            -1
        );
        assert_eq!(local, [42; 22]);
        assert_eq!(remote, [43; 22]);
        assert_eq!(
            conduit_z_tcp_endpoints(0, g, local.as_mut_ptr(), 22, remote.as_mut_ptr(), 22),
            0
        );
        assert_eq!(&local, b"255.255.255.255:65535\0");
        assert_eq!(remote, local);
        assert_eq!(conduit_z_tcp_close(0, g), 0);
        local.fill(42);
        remote.fill(43);
        assert_eq!(
            conduit_z_tcp_endpoints(0, g, local.as_mut_ptr(), 22, remote.as_mut_ptr(), 22),
            -1
        );
        assert_eq!(local, [42; 22]);
        assert_eq!(remote, [43; 22]);
    }
}
