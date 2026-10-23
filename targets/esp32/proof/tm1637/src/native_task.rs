//! Real single-executor task custody. Ready means the task owns its prepared
//! resources, never that a Body started. No Step is polled before release.
use crate::{
    native_root::DeviceExecution,
    native_worker::{Exit, Outcome, Progress},
};
use alloc::boxed::Box;
use core::cell::RefCell;
use embassy_executor::Spawner;
use embassy_sync::{
    blocking_mutex::{raw::CriticalSectionRawMutex, Mutex},
    signal::Signal,
};
use static_cell::StaticCell;

struct Slot {
    execution: RefCell<Option<Box<DeviceExecution>>>,
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
    pub context_bytes: usize,
    pub held_future_bytes: usize,
    pub source_sequence: u64,
    pub publication_sequence: u64,
    pub publication_channel: Option<u8>,
    pub publication_bytes: usize,
    pub committed_millis: Option<u64>,
    pub projection_refused: bool,
    pub steps: u64,
    pub submitted: [u64; 2],
    pub completed: [u64; 2],
    pub failure: Option<&'static str>,
    pub cleanup_failure: bool,
    pub native_live: bool,
    pub native_lifecycles: Option<[&'static str; 2]>,
    pub native_in_flight: Option<[u16; 2]>,
}
struct State {
    status: Status,
    canonical: heapless::Vec<u8, 4096>,
}
static STATE: Mutex<CriticalSectionRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State {
    status: Status {
        phase: "unregistered",
        context_bytes: 0,
        held_future_bytes: 0,
        source_sequence: 0,
        publication_sequence: 0,
        publication_channel: None,
        publication_bytes: 0,
        committed_millis: None,
        projection_refused: false,
        steps: 0,
        submitted: [0; 2],
        completed: [0; 2],
        failure: None,
        cleanup_failure: false,
        native_live: false,
        native_lifecycles: None,
        native_in_flight: None,
    },
    canonical: heapless::Vec::new(),
}));
fn update(f: impl FnOnce(&mut State)) {
    STATE.lock(|state| f(&mut state.borrow_mut()));
}
/// Exclusive Root handle to one actual task registration. It cannot be cloned
/// or deserialized into a second task, reservation, or release authority.
pub struct Handle {
    _slot: &'static Slot,
}
impl Handle {
    pub async fn wait_ready(&self) {
        READY.wait().await;
    }
    pub fn status(&self) -> Status {
        STATE.lock(|state| state.borrow().status)
    }
    pub fn abort_preparation(&self) {
        COMMAND.signal(Command::Cancel);
    }
    /// Copy under the short critical section; parsing/Mask work happens outside.
    pub fn copy_publication(&self, output: &mut [u8]) -> Result<Option<Status>, &'static str> {
        STATE.lock(|state| {
            let state = state.borrow();
            if state.status.publication_sequence == 0 {
                return Ok(None);
            }
            let target = output
                .get_mut(..state.canonical.len())
                .ok_or("publication-capacity")?;
            target.copy_from_slice(&state.canonical);
            Ok(Some(state.status))
        })
    }
}
pub fn prepare(
    execution: &mut Option<Box<DeviceExecution>>,
    spawner: Spawner,
) -> Result<Handle, &'static str> {
    if execution.is_none() {
        return Err("execution-context-unavailable");
    }
    let slot = SLOT
        .try_init_with(|| Slot {
            execution: RefCell::new(None),
        })
        .ok_or("worker-slot-already-used")?;
    slot.execution.replace(execution.take());
    update(|state| {
        state.status.phase = "registering";
        state.status.context_bytes = core::mem::size_of::<DeviceExecution>();
        state.status.held_future_bytes = core::mem::size_of_val(&held(slot));
    });
    match worker(slot) {
        Ok(token) => {
            spawner.spawn(token);
            Ok(Handle { _slot: slot })
        }
        Err(_) => {
            *execution = slot.execution.borrow_mut().take();
            update(|state| state.status.phase = "registration-refused");
            Err("worker-task-pool-unavailable")
        }
    }
}
#[embassy_executor::task]
async fn worker(slot: &'static Slot) {
    held(slot).await;
}
async fn held(slot: &'static Slot) {
    let Some(mut execution) = slot.execution.borrow_mut().take() else {
        update(|state| state.status.phase = "ownership-refused");
        return;
    };
    update(|state| {
        state.status.phase = "ready-held";
        native_status(&mut state.status, &execution);
    });
    READY.signal(());
    let outcome = match COMMAND.wait().await {
        Command::Cancel => {
            let cleanup =
                crate::native_service::quiesce(&mut execution.storage, &mut execution.native);
            Outcome {
                exit: Exit::Cancelled,
                cleanup,
                progress: Progress::default(),
            }
        }
        Command::Release => {
            update(|state| state.status.phase = "running");
            execution
                .run(
                    async {
                        let _ = COMMAND.wait().await;
                    },
                    |snapshot| {
                        update(|state| {
                            state.status.source_sequence = snapshot.sequence;
                            if snapshot.canonical.len() > state.canonical.capacity() {
                                state.status.projection_refused = true;
                                return;
                            }
                            state.canonical.clear();
                            state
                                .canonical
                                .extend_from_slice(snapshot.canonical)
                                .expect("preflighted publication projection capacity");
                            state.status.publication_bytes = snapshot.canonical.len();
                            state.status.publication_sequence = snapshot.sequence;
                            state.status.publication_channel = Some(snapshot.channel.index() as u8);
                            state.status.committed_millis =
                                Some(embassy_time::Instant::now().as_millis());
                            state.status.projection_refused = false;
                        });
                    },
                )
                .await
        }
    };
    update(|state| {
        state.status.phase = if outcome.cleanup.is_err() {
            "cleanup-refused"
        } else {
            match outcome.exit {
                Exit::Drained => "drained",
                Exit::Cancelled => "quiesced",
                Exit::Failed(_) => "failed",
            }
        };
        state.status.steps = outcome.progress.steps;
        state.status.submitted = outcome.progress.submitted;
        state.status.completed = outcome.progress.completed;
        state.status.failure = match outcome.exit {
            Exit::Failed(reason) => Some(reason.code.as_str()),
            _ => None,
        };
        state.status.cleanup_failure = outcome.cleanup.is_err();
        native_status(&mut state.status, &execution);
    });
    // Retain the actual terminal kernel/providers/evidence. This task registration
    // cannot be recycled into a fresh Play or claim automatic restart recovery.
    let _retained = execution;
    core::future::pending::<()>().await;
}

fn native_status(status: &mut Status, execution: &DeviceExecution) {
    use conduit_core::CapabilityLifecycle as L;
    let pulse = execution.native.pulse.inspection();
    let wait = execution.native.wait.inspection();
    let name = |life| match life {
        L::Issued => "issued",
        L::Revoked => "revoked",
        L::Exhausted => "exhausted",
    };
    status.native_live = pulse.lifecycle == L::Issued && wait.lifecycle == L::Issued;
    status.native_lifecycles = Some([name(pulse.lifecycle), name(wait.lifecycle)]);
    status.native_in_flight = Some([pulse.in_flight_operations, wait.in_flight_operations]);
}
