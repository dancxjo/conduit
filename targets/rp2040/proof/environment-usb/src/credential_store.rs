//! Diagnostic private credential persistence, not an installed Body storage Base.
//! Only the combined local I/O task owns the FLASH/CORE1 tokens. Requests reuse
//! canonical JOIN bytes; durable credential completion is separate from radio join.
use conduit_monotonic_wait::flash_window::Gate;
use conduit_network_join_call::credential_journal::{Journal, Refusal};
use core::cell::RefCell;
use embassy_rp::{
    flash::{Blocking, Flash},
    peripherals::CORE1,
    Peri,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{with_timeout, Duration, Instant};
#[path = "credential_store/hardware.rs"]
mod hardware;
#[path = "credential_store/policy.rs"]
mod policy;
#[path = "credential_store/wait.rs"]
mod wait;
type Device = Flash<
    'static,
    embassy_rp::peripherals::FLASH,
    Blocking,
    { conduit_rp2040_flash_layout::FLASH_BYTES as usize },
>;
const MAX: usize = conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize;
pub(super) struct Wire {
    pub bytes: [u8; MAX],
    pub length: usize,
}
impl Drop for Wire {
    fn drop(&mut self) {
        crate::protocol::wipe(&mut self.bytes);
    }
}
struct Request {
    wire: Wire,
    token: u64,
    deadline: Instant,
}
#[derive(Clone, Copy)]
struct Metrics {
    ready: bool,
    restored: bool,
    revision: u64,
    commits: u32,
    deferred: u32,
    maximum_us: u64,
}
struct State {
    owner: Option<Owner>,
    restored: Option<Wire>,
    generation: u64,
    metrics: Metrics,
}
static STATE: critical_section::Mutex<RefCell<State>> =
    critical_section::Mutex::new(RefCell::new(State {
        owner: None,
        restored: None,
        generation: 0,
        metrics: Metrics {
            ready: false,
            restored: false,
            revision: 0,
            commits: 0,
            deferred: 0,
            maximum_us: 0,
        },
    }));
static REQUEST: Signal<CriticalSectionRawMutex, Request> = Signal::new();
static RESPONSE: Signal<CriticalSectionRawMutex, (u64, Result<(), &'static str>)> = Signal::new();
pub(super) struct Owner {
    flash: Device,
    // Actual possession prevents later code from starting the second core.
    _core1: Peri<'static, CORE1>,
    gate: Gate,
    ready: bool,
    pending: Option<Request>,
}
/// Before any local/radio task starts. No erase/program path is entered here.
pub(super) fn initialize(mut flash: Device, core1: Peri<'static, CORE1>) {
    let profile = policy::reviewed_profile(
        flash.blocking_jedec_id().ok(),
        embassy_rp::clocks::clk_sys_freq(),
    );
    let mut boot = Wire {
        bytes: [0; MAX],
        length: 0,
    };
    let mut revision = 0;
    let restored = match Journal::open(hardware::Access::read_only(&mut flash)) {
        Ok(mut journal) => match journal.read(&mut boot.bytes) {
            Ok((selected_revision, length))
                if conduit_net::decode_network_join_request(&boot.bytes[..length]).is_ok() =>
            {
                revision = selected_revision;
                boot.length = length;
                Ok(Some(boot))
            }
            Err(Refusal::Empty) => Ok(None),
            _ => Err(()),
        },
        Err(_) => Err(()),
    };
    let ready = profile && restored.is_ok();
    let owner = Owner {
        flash,
        _core1: core1,
        gate: Gate::new(2000, 100, hardware::BUDGET_MS).expect("fixed local wait bounds"),
        ready,
        pending: None,
    };
    critical_section::with(|cs| {
        let mut state = STATE.borrow(cs).borrow_mut();
        assert!(state.owner.is_none(), "FLASH already owned");
        state.metrics.ready = ready;
        state.metrics.revision = revision;
        state.metrics.restored = revision != 0;
        state.owner = Some(owner);
        state.restored = restored.ok().flatten();
    });
}
pub(super) fn take_owner() -> Option<Owner> {
    critical_section::with(|cs| STATE.borrow(cs).borrow_mut().owner.take())
}
pub(super) fn restored() -> Option<Wire> {
    critical_section::with(|cs| STATE.borrow(cs).borrow_mut().restored.take())
}
pub(super) async fn commit(bytes: &[u8]) -> Result<(), &'static str> {
    conduit_net::decode_network_join_request(bytes).map_err(|_| "credential-request-invalid")?;
    let token = critical_section::with(|cs| {
        let mut state = STATE.borrow(cs).borrow_mut();
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or("credential-generation-exhausted")?;
        Ok::<_, &'static str>(state.generation)
    })?;
    let mut wire = Wire {
        bytes: [0; MAX],
        length: bytes.len(),
    };
    wire.bytes[..bytes.len()].copy_from_slice(bytes);
    REQUEST.signal(Request {
        wire,
        token,
        deadline: Instant::now()
            .checked_add(Duration::from_secs(6))
            .ok_or("credential-clock-exhausted")?,
    });
    with_timeout(Duration::from_secs(6), async {
        loop {
            let (received, result) = RESPONSE.wait().await;
            if received == token {
                return result;
            }
        }
    })
    .await
    .map_err(|_| "credential-commit-timeout")?
}

pub(super) fn append(out: &mut impl core::fmt::Write) -> core::fmt::Result {
    let m = critical_section::with(|cs| STATE.borrow(cs).borrow().metrics);
    write!(out, ",\"credentials\":{{\"ready\":{},\"restored\":{},\"revision\":{},\"commits\":{},\"deferred\":{},\"max_us\":{}}}", m.ready, m.restored, m.revision, m.commits, m.deferred, m.maximum_us)
}
