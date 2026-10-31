//! Explicit carrier experiment: no Body membership, protected Line or commit ACK.
use conduit_zenoh_pico_embassy::{
    CArena, CallbackPlatform, Callbacks, SocketOwner, SocketWorker, WorkerRuntime,
};
use conduit_zenoh_pico_platform::{Endpoint, Fault, SocketBridge};
use core::{cell::RefCell, ffi::c_void, fmt::Write, mem::MaybeUninit};
use critical_section::Mutex;
use embassy_net::{tcp::TcpSocket, IpAddress, Stack};
use embassy_rp::{
    multicore::{self, Stack as CoreStack},
    peripherals::CORE1,
    Peri,
};
use embassy_time::{block_for, Duration, Instant, Timer};
use static_cell::StaticCell;
mod construction {
    include!(concat!(env!("OUT_DIR"), "/zenoh.rs"));
}
const RING: usize = 256;
const TCP: usize = 256;
const STACK: usize = 16 * 1024;
static BRIDGE: Mutex<RefCell<SocketBridge<RING, RING>>> =
    Mutex::new(RefCell::new(SocketBridge::new()));
static CORE_STACK: StaticCell<CoreStack<STACK>> = StaticCell::new();
#[derive(Clone, Copy)]
struct Snapshot {
    worker_phase: u8,
    link_polls: u32,
    link: bool,
    endpoints: Option<(u32, Endpoint, Endpoint)>,
    opened: bool,
    publications: u64,
    result: i32,
    prepared_us: u64,
    began_us: u64,
    used: usize,
    peak: usize,
    metrics_valid: bool,
}
static STATE: Mutex<RefCell<Snapshot>> = Mutex::new(RefCell::new(Snapshot {
    worker_phase: 0,
    link_polls: 0,
    link: false,
    endpoints: None,
    opened: false,
    publications: 0,
    result: 0,
    prepared_us: 0,
    began_us: 0,
    used: 0,
    peak: 0,
    metrics_valid: false,
}));
fn update(f: impl FnOnce(&mut Snapshot)) {
    critical_section::with(|cs| f(&mut STATE.borrow(cs).borrow_mut()));
}
#[derive(Clone, Copy)]
struct Platform;
impl WorkerRuntime for Platform {
    fn now_ms(&self) -> u64 {
        Instant::now().as_millis()
    }
    fn yield_worker(&self) -> Result<(), Fault> {
        block_for(Duration::from_millis(1));
        Ok(())
    }
}
impl CallbackPlatform for Platform {
    fn boot_us(&self) -> u64 {
        Instant::now().as_micros()
    }
    fn sleep_us(&self, n: u64) -> Result<(), Fault> {
        if n > 1_000_000 {
            return Err(Fault::TimedOut);
        }
        update(|s| s.worker_phase = 5);
        block_for(Duration::from_micros(n));
        update(|s| s.worker_phase = 6);
        Ok(())
    }
    fn random_fill(&mut self, bytes: &mut [u8]) -> Result<(), Fault> {
        // Hardware ROSC entropy for this non-authoritative transport session;
        // no Body identity, signing or owner key is generated here.
        embassy_rp::clocks::RoscRng.fill_bytes(bytes);
        Ok(())
    }
    fn wall_time(&self) -> Option<(u32, u32)> {
        None
    }
    fn endpoints(&self) -> Option<(u32, Endpoint, Endpoint)> {
        critical_section::with(|cs| STATE.borrow(cs).borrow().endpoints)
    }
    fn entropy_failure(&self) -> ! {
        update(|s| {
            s.result = -900;
            s.metrics_valid = false;
        });
        // Park only this worker. Its arena/context stays retained; no global
        // panic, reboot, critical-section guard or premature reclamation.
        loop {
            cortex_m::asm::wfe();
        }
    }
}
type Context = Callbacks<
    'static,
    Mutex<RefCell<SocketBridge<RING, RING>>>,
    Platform,
    Platform,
    RING,
    RING,
    16,
>;
static mut CONTEXT: Option<Context> = None;
fn with_context<T>(f: impl FnOnce(&mut Context) -> T) -> T {
    assert_eq!(multicore::current_core(), multicore::CoreId::Core1);
    // SAFETY: initialized by the unique core1 entry before C starts. C is
    // single-threaded and non-reentrant; core0 shares only BRIDGE/STATE, never
    // this context. No interrupt mask or cross-core lock spans this closure.
    unsafe {
        f((*core::ptr::addr_of_mut!(CONTEXT))
            .as_mut()
            .expect("worker initialized"))
    }
}
conduit_zenoh_pico_embassy::export_zenoh_pico_callbacks!(with_context);
unsafe extern "C" {
    fn conduit_evaluation_run(locator: *const u8) -> i32;
    fn conduit_evaluation_put(
        session: *mut c_void,
        key: *const c_void,
        bytes: *const u8,
        len: usize,
    ) -> i32;
}
pub fn start(core: Peri<'static, CORE1>, backing: &'static mut [MaybeUninit<u8>]) {
    let arena = CArena::new(backing).expect("explicit finite foreign partition");
    let stack = CORE_STACK.init_with(CoreStack::new);
    multicore::spawn_core1(core, stack, move || {
        update(|s| s.worker_phase = 1);
        let worker = SocketWorker::new(&BRIDGE, Platform, 1000).unwrap();
        // SAFETY: only this unique core writes/uses the context, before C entry.
        unsafe {
            core::ptr::addr_of_mut!(CONTEXT).write(Some(Callbacks::new(worker, arena, Platform)));
        }
        update(|s| s.worker_phase = 2);
        let result = unsafe { conduit_evaluation_run(construction::LOCATOR.as_ptr()) };
        conduit_evaluation_event(4, result);
        loop {
            cortex_m::asm::wfe();
        }
    });
}
#[embassy_executor::task]
pub async fn network(stack: Stack<'static>) -> ! {
    let mut rx = [0; TCP];
    let mut tx = [0; TCP];
    let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
    let mut owner = SocketOwner::<128>::new();
    loop {
        owner.poll(&mut socket, &BRIDGE, Instant::now().as_millis());
        let endpoints = owner.endpoints().map(|(g, l, r)| {
            let IpAddress::Ipv4(lip) = l.addr;
            let IpAddress::Ipv4(rip) = r.addr;
            (
                g.get(),
                Endpoint {
                    ipv4: lip.octets(),
                    port: l.port,
                },
                Endpoint {
                    ipv4: rip.octets(),
                    port: r.port,
                },
            )
        });
        update(|s| {
            s.link = stack.is_link_up() && stack.config_v4().is_some();
            s.endpoints = endpoints;
        });
        Timer::after_millis(1).await;
    }
}
#[unsafe(no_mangle)]
extern "C" fn conduit_evaluation_link_up() -> i32 {
    let link = critical_section::with(|cs| {
        let mut state = STATE.borrow(cs).borrow_mut();
        state.worker_phase = 3;
        state.link_polls = state.link_polls.saturating_add(1);
        state.link
    });
    i32::from(link)
}
#[unsafe(no_mangle)]
extern "C" fn conduit_evaluation_event(event: u32, result: i32) {
    let stats = with_context(|c| c.arena_stats());
    let now = Instant::now().as_micros();
    update(|s| {
        s.worker_phase = 7;
        s.used = stats.used;
        s.peak = stats.peak_used;
        s.metrics_valid = true;
        if event == 1 {
            s.began_us = now;
        }
        if event == 2 {
            s.opened = result >= 0;
            s.prepared_us = now.saturating_sub(s.began_us);
        }
        if event == 3 && result >= 0 {
            s.publications = s.publications.saturating_add(1);
        }
        s.result = result;
    });
}
#[unsafe(no_mangle)]
unsafe extern "C" fn conduit_evaluation_publish(session: *mut c_void, key: *const c_void) -> i32 {
    let Ok(payload) = crate::environment::status() else {
        return -901;
    };
    // No mutable callback-context borrow spans this C call, which allocates
    // through the exported arena callbacks. The C-owned handles stay opaque.
    unsafe { conduit_evaluation_put(session, key, payload.as_ptr(), payload.len()) }
}
pub fn status(out: &mut impl Write) -> core::fmt::Result {
    let s = critical_section::with(|cs| *STATE.borrow(cs).borrow());
    write!(
        out,
        ",\"zenoh_worker_phase\":{},\"zenoh_link_polls\":{}",
        s.worker_phase, s.link_polls
    )?;
    write!(out,",\"zenoh_evaluation\":{{\"link\":{},\"opened\":{},\"accepted_publications\":{},\"last_result\":{},\"prepared_us\":{},\"arena_used\":{},\"arena_peak\":{},\"metrics_valid\":{},\"rust_heap_capacity\":{},\"worker_stack_reserved\":{STACK}}}",s.link,s.opened,s.publications,s.result,s.prepared_us,s.used,s.peak,s.metrics_valid,crate::heap::capacity())
}
