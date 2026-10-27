//! Actual held executor custody for the ordinary timed Mask. Registration is
//! not Body start; activation and the exclusive release handle are separate.
use super::session::NativeMaskSession;
use super::worker::workspace;
static WORKSPACE: workspace::Gate = workspace::Gate::new();
use crate::native_authority::{MaskLeaseIssuer, MaskScopeGuard};
use crate::native_mask_evidence::Consumer;
use alloc::boxed::Box;
use core::cell::RefCell;
use embassy_executor::Spawner;
use embassy_sync::{
    blocking_mutex::{raw::CriticalSectionRawMutex, Mutex},
    signal::Signal,
};
use static_cell::StaticCell;
struct Slot {
    session: RefCell<Option<Box<NativeMaskSession>>>,
    leases: RefCell<Option<Box<MaskLeaseIssuer>>>,
    evidence: RefCell<Option<Box<Consumer>>>,
    revoke: MaskScopeGuard,
}
static SLOT: StaticCell<Slot> = StaticCell::new();
#[derive(Clone, Copy)]
enum Command {
    Release,
    Cancel,
}
static COMMAND: Signal<CriticalSectionRawMutex, Command> = Signal::new();
static READY: Signal<CriticalSectionRawMutex, ()> = Signal::new();
#[derive(Clone, Copy, serde::Serialize)]
pub struct Status {
    pub phase: &'static str,
    pub task_active: bool,
    pub activated: bool,
    pub context_bytes: usize,
    pub held_future_bytes: usize,
    pub run_future_bytes: usize,
    pub completed: u64,
    pub pending_evidence: bool,
    pub failure: Option<&'static str>,
    pub cleanup_refusal: Option<&'static str>,
}
struct State {
    cancelled: bool,
    status: Status,
}
static STATE: Mutex<CriticalSectionRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State {
    cancelled: false,
    status: Status {
        phase: "unregistered",
        task_active: false,
        activated: false,
        context_bytes: 0,
        held_future_bytes: 0,
        run_future_bytes: 0,
        completed: 0,
        pending_evidence: false,
        failure: None,
        cleanup_refusal: None,
    },
}));
fn update(f: impl FnOnce(&mut State)) {
    STATE.lock(|state| f(&mut state.borrow_mut()));
}
pub struct Handle {
    slot: &'static Slot,
}
pub struct Retired {
    pub session: Box<NativeMaskSession>,
    pub leases: Box<MaskLeaseIssuer>,
    pub evidence: Box<Consumer>,
}
impl Handle {
    /// Scheduling custody only. Neither this request nor its permit admits a
    /// network kernel or replaces current Body/capacity checks. Drop withdraws
    /// the exact request, including while waiting for worker retirement.
    pub(crate) fn request_workspace(&self) -> Result<workspace::Request<'static>, &'static str> {
        let status = self.status();
        if !status.task_active || status.phase != "running" {
            return Err("mask-workspace-not-running");
        }
        WORKSPACE.request()
    }

    pub async fn wait_ready(&self) {
        READY.wait().await;
    }
    pub fn status(&self) -> Status {
        STATE.lock(|state| state.borrow().status)
    }
    /// Only the actual started/checkpointed Body is borrowed synchronously. No
    /// Body borrow, mutable Root or received identity enters the task future.
    pub fn activate(
        &self,
        body: &conduit_body::SealedBodyLifecycleSession,
        now: u64,
    ) -> Result<(), &'static str> {
        let status = self.status();
        if status.phase != "ready-held" || status.activated {
            return Err("mask-task-not-held");
        }
        self.slot
            .leases
            .borrow_mut()
            .as_mut()
            .ok_or("mask-task-issuer")?
            .activate(body, now)?;
        update(|state| state.status.activated = true);
        Ok(())
    }
    pub fn accept_start(&self) -> Result<(), &'static str> {
        STATE.lock(|state| {
            let mut state = state.borrow_mut();
            let ready = state.status.activated
                && state.status.completed == 0
                && !state.status.pending_evidence;
            let cancelled = state.cancelled;
            crate::native_execution_gate::accept_start(&mut state.status.phase, cancelled, ready)
        })
    }
    pub fn release(&self) -> Result<(), &'static str> {
        STATE.lock(|state| {
            let mut state = state.borrow_mut();
            let cancelled = state.cancelled;
            crate::native_execution_gate::release(&mut state.status.phase, cancelled)?;
            COMMAND.signal(Command::Release);
            Ok(())
        })
    }
    pub fn abort_preparation(&self) {
        self.slot.revoke.revoke();
        WORKSPACE.cancel();
        update(|state| {
            state.cancelled = true;
            if state.status.task_active {
                state.status.phase = "cancellation-requested";
            }
        });
        COMMAND.signal(Command::Cancel);
    }
    pub fn retry_evidence(&self) -> Result<(), &'static str> {
        let status = self.status();
        if status.task_active || status.phase != "cleanup-refused" || !status.pending_evidence {
            return Err("mask-no-pending-evidence");
        }
        let mut session = self.slot.session.borrow_mut();
        let mut leases = self.slot.leases.borrow_mut();
        let mut evidence = self.slot.evidence.borrow_mut();
        let session = session.as_mut().ok_or("mask-task-session")?;
        session.accept_pending(
            &mut **leases.as_mut().ok_or("mask-task-issuer")?,
            &mut **evidence.as_mut().ok_or("mask-task-evidence")?,
        )?;
        update(|state| {
            state.status.completed = session.completed();
            state.status.pending_evidence = false;
            state.status.failure = None;
            state.status.cleanup_refusal = None;
            state.status.phase = "quiesced";
        });
        Ok(())
    }
    pub fn retire(&self) -> Result<Retired, &'static str> {
        let status = self.status();
        if status.task_active || status.phase != "quiesced" || status.pending_evidence {
            return Err("mask-task-not-quiesced");
        }
        let mut session = self.slot.session.borrow_mut();
        let mut leases = self.slot.leases.borrow_mut();
        let mut evidence = self.slot.evidence.borrow_mut();
        if session.is_none() || leases.is_none() || evidence.is_none() {
            return Err("mask-task-custody");
        }
        let retired = Retired {
            session: session.take().ok_or("mask-task-session")?,
            leases: leases.take().ok_or("mask-task-issuer")?,
            evidence: evidence.take().ok_or("mask-task-evidence")?,
        };
        update(|state| state.status.phase = "retired");
        Ok(retired)
    }
}
pub fn prepare(
    session: &mut Option<Box<NativeMaskSession>>,
    leases: &mut Option<Box<MaskLeaseIssuer>>,
    evidence: &mut Option<Box<Consumer>>,
    spawner: Spawner,
) -> Result<Handle, &'static str> {
    if session.is_none() || leases.is_none() || evidence.is_none() {
        return Err("mask-task-owners");
    }
    let revoke = leases
        .as_ref()
        .ok_or("mask-task-issuer")?
        .revocation_guard();
    let slot = SLOT
        .try_init_with(|| Slot {
            session: RefCell::new(None),
            leases: RefCell::new(None),
            evidence: RefCell::new(None),
            revoke,
        })
        .ok_or("mask-task-already-used")?;
    slot.session.replace(session.take());
    slot.leases.replace(leases.take());
    slot.evidence.replace(evidence.take());
    update(|state| {
        state.status.phase = "registering";
        state.status.task_active = true;
        state.status.context_bytes = core::mem::size_of::<NativeMaskSession>();
        state.status.held_future_bytes = core::mem::size_of_val(&held(slot));
    });
    match task(slot) {
        Ok(token) => {
            spawner.spawn(token);
            Ok(Handle { slot })
        }
        Err(_) => {
            *session = slot.session.borrow_mut().take();
            *leases = slot.leases.borrow_mut().take();
            *evidence = slot.evidence.borrow_mut().take();
            update(|state| {
                state.status.phase = "registration-refused";
                state.status.task_active = false;
            });
            Err("mask-task-pool")
        }
    }
}
// Exact stable expansion from pinned embassy-executor0.10.0 / macros0.8.0,
// with only the initialized pool's placement changed. CPU0 awake data only;
// neither DMA, second-core access nor persistence across sleep is admitted.
fn task(
    slot: &'static Slot,
) -> Result<embassy_executor::SpawnToken<impl Sized>, embassy_executor::SpawnError> {
    const POOL_SIZE: usize = 1;
    #[cfg_attr(feature = "local-appliance", esp_hal::ram(unstable(rtc_slow)))]
    static POOL: embassy_executor::_export::TaskPoolHolder<
        { embassy_executor::_export::task_pool_size::<_, _, _, POOL_SIZE>(held) },
        { embassy_executor::_export::task_pool_align::<_, _, _, POOL_SIZE>(held) },
    > = unsafe {
        // SAFETY: the exact upstream initialized layout cast, with both size
        // and alignment inferred from this same held async function.
        core::mem::transmute(embassy_executor::_export::task_pool_new::<_, _, _, POOL_SIZE>(held))
    };
    const fn get<F, Args, Fut>(_: F) -> &'static embassy_executor::raw::TaskPool<Fut, POOL_SIZE>
    where
        F: embassy_executor::_export::TaskFn<Args, Fut = Fut>,
        Fut: core::future::Future + 'static,
    {
        // SAFETY: upstream Holder guarantees layout; the same function is used
        // for initialization, inference and in-place exclusive pool spawning.
        unsafe { &*POOL.get().cast() }
    }
    // SAFETY: exactly an async-function constructor, as required by the pinned
    // API; preserve its Send bookkeeping and in-place initialization path.
    unsafe { get(held)._spawn_async_fn(move || held(slot)) }
}
async fn held(slot: &'static Slot) {
    update(|state| {
        state.status.phase = if state.cancelled {
            "cancellation-requested"
        } else {
            "ready-held"
        }
    });
    READY.signal(());
    let command = COMMAND.wait().await;
    if matches!(command, Command::Cancel) {
        slot.revoke.revoke();
        update(|state| {
            state.status.phase = "quiesced";
            state.status.task_active = false;
        });
        return;
    }
    let Some(mut session) = slot.session.borrow_mut().take() else {
        update(|state| {
            state.status.phase = "ownership-refused";
            state.status.task_active = false;
        });
        return;
    };
    let Some(mut leases) = slot.leases.borrow_mut().take() else {
        slot.session.replace(Some(session));
        update(|state| {
            state.status.phase = "ownership-refused";
            state.status.task_active = false;
        });
        return;
    };
    let Some(mut evidence) = slot.evidence.borrow_mut().take() else {
        slot.session.replace(Some(session));
        slot.leases.replace(Some(leases));
        update(|state| {
            state.status.phase = "ownership-refused";
            state.status.task_active = false;
        });
        return;
    };
    update(|state| state.status.phase = "running");
    let result = {
        let future = session.run_with_workspace(&mut *leases, &mut *evidence, Some(&WORKSPACE));
        update(|state| state.status.run_future_bytes = core::mem::size_of_val(&future));
        future.await
    };
    slot.revoke.revoke();
    WORKSPACE.cancel();
    update(|state| {
        state.status.completed = session.completed();
        state.status.pending_evidence = session.pending_evidence();
        state.status.failure = result.err();
        state.status.cleanup_refusal = session.cleanup_refusal();
        state.status.task_active = false;
        state.status.phase = if state.status.failure.is_some() {
            "cleanup-refused"
        } else {
            "quiesced"
        };
    });
    slot.session.replace(Some(session));
    slot.leases.replace(Some(leases));
    slot.evidence.replace(Some(evidence));
}
