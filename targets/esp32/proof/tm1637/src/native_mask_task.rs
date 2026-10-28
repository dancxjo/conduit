//! Actual held executor custody for the ordinary timed Mask. Registration is
//! not Body start; activation and the exclusive release handle are separate.
use super::session::NativeMaskSession;
pub use super::session::{RetiredMask, RetiredMaskParts};
use super::worker::workspace;
pub(crate) use workspace::{Permit as WorkspacePermit, Request as WorkspaceRequest};
static WORKSPACE: workspace::Gate = workspace::Gate::new();
use crate::native_authority::{MaskLeaseIssuer, MaskScopeGuard};
use crate::native_mask_evidence::Consumer;
use alloc::boxed::Box;
use core::cell::{Cell, RefCell};
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
    revoke: RefCell<Option<MaskScopeGuard>>,
    completed_retirement: Cell<bool>,
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
const EMPTY_STATUS: Status = Status {
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
};
static STATE: Mutex<CriticalSectionRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State {
    cancelled: false,
    status: EMPTY_STATUS,
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

    /// Actual stopped-task custody only; neither failure status nor electrical
    /// uncertainty is cleared. The gate also excludes retained-owner transfer.
    pub(crate) fn request_terminal_workspace(
        &self,
    ) -> Result<workspace::Request<'static>, &'static str> {
        let status = self.status();
        if status.task_active || !matches!(status.phase, "failed" | "cleanup-refused" | "quiesced")
        {
            return Err("mask-task-not-terminal");
        }
        let session = self
            .slot
            .session
            .try_borrow()
            .map_err(|_| "mask-session-busy")?;
        let leases = self
            .slot
            .leases
            .try_borrow()
            .map_err(|_| "mask-leases-busy")?;
        let evidence = self
            .slot
            .evidence
            .try_borrow()
            .map_err(|_| "mask-evidence-busy")?;
        WORKSPACE.terminal_boundary(session.is_some() && leases.is_some() && evidence.is_some())
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
        if !WORKSPACE.idle() {
            return Err("mask-workspace-held");
        }
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
        if !WORKSPACE.idle() {
            return Err("mask-workspace-held");
        }
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
        if !WORKSPACE.idle() {
            return Err("mask-workspace-held");
        }
        STATE.lock(|state| {
            let mut state = state.borrow_mut();
            let cancelled = state.cancelled;
            crate::native_execution_gate::release(&mut state.status.phase, cancelled)?;
            COMMAND.signal(Command::Release);
            Ok(())
        })
    }
    pub fn abort_preparation(&self) {
        if let Some(guard) = self.slot.revoke.borrow().as_ref() {
            guard.revoke();
        }
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
        if !WORKSPACE.idle() {
            return Err("mask-workspace-held");
        }
        let status = self.status();
        if status.task_active
            || !matches!(status.phase, "failed" | "cleanup-refused")
            || !status.pending_evidence
        {
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
        if !WORKSPACE.idle() {
            return Err("mask-workspace-held");
        }
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
    /// Actual complete custody, not a cancellation receipt or a replacement grant.
    pub fn retire_completed(self) -> Result<(Self, RetiredMask), (Self, &'static str)> {
        match self.take_completed() {
            Ok(retired) => Ok((self, retired)),
            Err(reason) => Err((self, reason)),
        }
    }
    fn take_completed(&self) -> Result<RetiredMask, &'static str> {
        if !WORKSPACE.idle() {
            return Err("mask-workspace-held");
        }
        let status = self.status();
        if status.task_active
            || status.phase != "quiesced"
            || !status.activated
            || status.completed != super::worker::MAXIMUM_CHILDREN
            || status.pending_evidence
            || status.failure.is_some()
            || status.cleanup_refusal.is_some()
        {
            return Err("mask-task-not-completed");
        }
        let mut session = self
            .slot
            .session
            .try_borrow_mut()
            .map_err(|_| "mask-task-session-busy")?;
        let mut leases = self
            .slot
            .leases
            .try_borrow_mut()
            .map_err(|_| "mask-task-issuer-busy")?;
        let mut evidence = self
            .slot
            .evidence
            .try_borrow_mut()
            .map_err(|_| "mask-task-evidence-busy")?;
        let mut guard = self
            .slot
            .revoke
            .try_borrow_mut()
            .map_err(|_| "mask-task-guard-busy")?;
        let current = session.as_ref().ok_or("mask-task-session")?;
        let issuer = leases.as_ref().ok_or("mask-task-issuer")?;
        let consumer = evidence.as_ref().ok_or("mask-task-evidence")?;
        if guard.is_none() {
            return Err("mask-task-guard");
        }
        current.retirement_ready(issuer, consumer)?;
        let result = session
            .take()
            .expect("preflighted session")
            .retire_completed(
                leases.take().expect("preflighted issuer"),
                evidence.take().expect("preflighted evidence"),
            );
        match result {
            Ok(retired) => {
                guard.take();
                self.slot.completed_retirement.set(true);
                update(|state| {
                    state.status.phase = "retired";
                    state.status.context_bytes = 0;
                });
                Ok(retired)
            }
            Err((returned_session, returned_leases, returned_evidence, reason)) => {
                *session = Some(returned_session);
                *leases = Some(returned_leases);
                *evidence = Some(returned_evidence);
                Err(reason)
            }
        }
    }
    /// Reuse this exact empty slot with independently admitted fresh owners.
    /// Workspace generations are preserved, and no old witness is reactivated.
    pub fn prepare_next(
        self,
        session: &mut Option<Box<NativeMaskSession>>,
        leases: &mut Option<Box<MaskLeaseIssuer>>,
        evidence: &mut Option<Box<Consumer>>,
        spawner: Spawner,
    ) -> Result<Self, (Self, &'static str)> {
        let status = self.status();
        if !WORKSPACE.idle()
            || status.phase != "retired"
            || status.task_active
            || !self.slot.completed_retirement.get()
            || self.slot.session.borrow().is_some()
            || self.slot.leases.borrow().is_some()
            || self.slot.evidence.borrow().is_some()
            || self.slot.revoke.borrow().is_some()
        {
            return Err((self, "mask-task-slot-not-completed"));
        }
        match register(self.slot, session, leases, evidence, spawner) {
            Ok(next) => Ok(next),
            Err(reason) => {
                self.slot.completed_retirement.set(true);
                update(|state| state.status.phase = "retired");
                Err((self, reason))
            }
        }
    }
}
pub fn prepare(
    session: &mut Option<Box<NativeMaskSession>>,
    leases: &mut Option<Box<MaskLeaseIssuer>>,
    evidence: &mut Option<Box<Consumer>>,
    spawner: Spawner,
) -> Result<Handle, &'static str> {
    if !session.as_ref().is_some_and(|s| s.fresh_for_task())
        || !leases.as_ref().is_some_and(|l| l.fresh_for_task())
        || !evidence
            .as_ref()
            .is_some_and(|e| e.completion() == (0, None))
    {
        return Err("mask-task-fresh-owners");
    }
    let slot = SLOT
        .try_init_with(|| Slot {
            session: RefCell::new(None),
            leases: RefCell::new(None),
            evidence: RefCell::new(None),
            revoke: RefCell::new(None),
            completed_retirement: Cell::new(false),
        })
        .ok_or("mask-task-already-used")?;
    register(slot, session, leases, evidence, spawner)
}
fn register(
    slot: &'static Slot,
    session: &mut Option<Box<NativeMaskSession>>,
    leases: &mut Option<Box<MaskLeaseIssuer>>,
    evidence: &mut Option<Box<Consumer>>,
    spawner: Spawner,
) -> Result<Handle, &'static str> {
    if !session.as_ref().is_some_and(|s| s.fresh_for_task())
        || !leases.as_ref().is_some_and(|l| l.fresh_for_task())
        || !evidence
            .as_ref()
            .is_some_and(|e| e.completion() == (0, None))
    {
        return Err("mask-task-fresh-owners");
    }
    if slot.session.borrow().is_some()
        || slot.leases.borrow().is_some()
        || slot.evidence.borrow().is_some()
        || slot.revoke.borrow().is_some()
    {
        return Err("mask-task-slot-retained");
    }
    let revoke = leases
        .as_ref()
        .expect("preflighted fresh issuer")
        .revocation_guard();
    COMMAND.reset();
    READY.reset();
    WORKSPACE.cancel();
    slot.completed_retirement.set(false);
    slot.revoke.replace(Some(revoke));
    slot.session.replace(session.take());
    slot.leases.replace(leases.take());
    slot.evidence.replace(evidence.take());
    update(|state| {
        state.cancelled = false;
        state.status = Status {
            phase: "registering",
            task_active: true,
            context_bytes: core::mem::size_of::<NativeMaskSession>(),
            held_future_bytes: core::mem::size_of_val(&held(slot)),
            ..EMPTY_STATUS
        };
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
            slot.revoke.borrow_mut().take();
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
    #[cfg_attr(
        all(target_os = "none", feature = "local-appliance"),
        esp_hal::ram(unstable(rtc_slow))
    )]
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
        if let Some(guard) = slot.revoke.borrow().as_ref() {
            guard.revoke();
        }
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
    if let Some(guard) = slot.revoke.borrow().as_ref() {
        guard.revoke();
    }
    WORKSPACE.cancel();
    update(|state| {
        state.status.completed = session.completed();
        state.status.pending_evidence = session.pending_evidence();
        state.status.failure = result.err();
        state.status.cleanup_refusal = session.cleanup_refusal();
        state.status.task_active = false;
        state.status.phase = if state.status.cleanup_refusal.is_some() {
            "cleanup-refused"
        } else if state.status.failure.is_some() {
            "failed"
        } else {
            "quiesced"
        };
    });
    slot.session.replace(Some(session));
    slot.leases.replace(Some(leases));
    slot.evidence.replace(Some(evidence));
}
