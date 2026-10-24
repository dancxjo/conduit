//! Real single-executor task custody. Ready means the task owns its prepared
//! resources, never that a Body started. No Step is polled before release.
use crate::{native_root::DeviceExecution, native_worker::Exit};
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
    /// Static task-pool reservation remains present even after the task exits.
    pub held_future_bytes: usize,
    pub task_active: bool,
    pub source_sequence: u64,
    pub publication_sequence: u64,
    pub publication_channel: Option<u8>,
    pub publication_bytes: usize,
    pub committed_millis: Option<u64>,
    pub projection_refused: bool,
    /// Counters are exact while held/terminal, not sampled from active futures.
    pub progress_current: bool,
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
    cancel_requested: bool,
    status: Status,
}
const PROJECTION_BYTES: usize = 4096;
// Reclaimed backing is initialized only by committed publication copies. Access
// is serialized by STATE; publication_bytes records the initialized prefix.
#[esp_hal::ram(reclaimed)]
static mut CANONICAL: core::mem::MaybeUninit<[u8; PROJECTION_BYTES]> =
    core::mem::MaybeUninit::uninit();
static STATE: Mutex<CriticalSectionRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State {
    cancel_requested: false,
    status: Status {
        phase: "unregistered",
        context_bytes: 0,
        held_future_bytes: 0,
        task_active: false,
        source_sequence: 0,
        publication_sequence: 0,
        publication_channel: None,
        publication_bytes: 0,
        committed_millis: None,
        projection_refused: false,
        progress_current: true,
        steps: 0,
        submitted: [0; 2],
        completed: [0; 2],
        failure: None,
        cleanup_failure: false,
        native_live: false,
        native_lifecycles: None,
        native_in_flight: None,
    },
}));
fn update(f: impl FnOnce(&mut State)) {
    STATE.lock(|state| f(&mut state.borrow_mut()));
}
/// Exclusive Root handle to one actual task registration. It cannot be cloned
/// or deserialized into a second task, reservation, or release authority.
pub struct Handle {
    slot: &'static Slot,
}
impl Handle {
    pub async fn wait_ready(&self) {
        READY.wait().await;
    }
    pub fn status(&self) -> Status {
        STATE.lock(|state| state.borrow().status)
    }
    pub fn accept_start(&self) -> Result<(), &'static str> {
        STATE.lock(|state| {
            let mut state = state.borrow_mut();
            if state.cancel_requested {
                return Err("worker-cancellation-requested");
            }
            let status = &mut state.status;
            if status.phase != "ready-held"
                || !status.native_live
                || status.native_in_flight != Some([0, 0])
                || status.native_lifecycles != Some(["issued", "issued"])
                || !status.progress_current
                || status.steps != 0
                || status.publication_sequence != 0
            {
                return Err("worker-not-ready-held");
            }
            status.phase = "started-held";
            Ok(())
        })
    }
    pub fn release(&self) -> Result<(), &'static str> {
        STATE.lock(|state| {
            let mut state = state.borrow_mut();
            if state.cancel_requested {
                return Err("worker-cancellation-requested");
            }
            let status = &mut state.status;
            if status.phase != "started-held" {
                return Err("worker-not-started-held");
            }
            status.phase = "released";
            COMMAND.signal(Command::Release);
            Ok(())
        })
    }
    pub fn abort_preparation(&self) {
        let signal = STATE.lock(|state| {
            let mut state = state.borrow_mut();
            if matches!(
                state.status.phase,
                "quiesced"
                    | "drained"
                    | "failed"
                    | "cleanup-refused"
                    | "ownership-refused"
                    | "retired"
            ) {
                return false;
            }
            state.cancel_requested = true;
            state.status.phase = "cancellation-requested";
            true
        });
        if signal {
            COMMAND.signal(Command::Cancel);
        }
    }
    /// Only Root's exact authenticated retirement may take terminal custody.
    pub fn retire(
        &self,
    ) -> Result<
        crate::native_worker::RetiredProviders<
            conduit_esp32_pulse_capture::Esp32PulseCapture<'static>,
            crate::native_wait::EspWait,
        >,
        &'static str,
    > {
        let status = self.status();
        if status.phase != "quiesced" || status.task_active {
            return Err("worker-not-quiesced");
        }
        let providers = DeviceExecution::retire_retained(&mut self.slot.execution.borrow_mut())?;
        update(|state| {
            state.status.phase = "retired";
            state.status.context_bytes = 0;
        });
        Ok(providers)
    }
    /// Copy under the short critical section; parsing/Mask work happens outside.
    pub fn copy_publication(&self, output: &mut [u8]) -> Result<Option<Status>, &'static str> {
        STATE.lock(|state| {
            let state = state.borrow();
            if state.status.publication_sequence == 0 {
                return Ok(None);
            }
            let target = output
                .get_mut(..state.status.publication_bytes)
                .ok_or("publication-capacity")?;
            // SAFETY: STATE serializes all backing access. This prefix was
            // initialized before publication_sequence became nonzero, and the
            // caller's ordinary output slice cannot alias private static memory.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    core::ptr::addr_of!(CANONICAL).cast::<u8>(),
                    target.as_mut_ptr(),
                    target.len(),
                );
            }
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
        state.status.task_active = true;
        state.status.context_bytes = core::mem::size_of::<DeviceExecution>();
        state.status.held_future_bytes = core::mem::size_of_val(&held(slot));
    });
    match worker(slot) {
        Ok(token) => {
            spawner.spawn(token);
            Ok(Handle { slot: slot })
        }
        Err(_) => {
            *execution = slot.execution.borrow_mut().take();
            update(|state| {
                state.status.phase = "registration-refused";
                state.status.task_active = false;
            });
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
        update(|state| {
            state.status.phase = "ownership-refused";
            state.status.task_active = false;
        });
        return;
    };
    update(|state| {
        state.status.phase = if state.cancel_requested {
            "cancellation-requested"
        } else {
            "ready-held"
        };
        native_status(&mut state.status, &execution);
    });
    READY.signal(());
    let outcome = match COMMAND.wait().await {
        Command::Cancel => execution.cancel_held().expect("held task has never run"),
        Command::Release => {
            update(|state| {
                state.status.phase = "running";
                state.status.progress_current = false;
                state.status.native_in_flight = None;
            });
            execution
                .run(
                    async {
                        let _ = COMMAND.wait().await;
                    },
                    |snapshot| {
                        update(|state| {
                            state.status.source_sequence = snapshot.sequence;
                            if snapshot.canonical.len() > PROJECTION_BYTES {
                                state.status.projection_refused = true;
                                return;
                            }
                            // SAFETY: bounded above and serialized by STATE;
                            // the kernel snapshot cannot alias this private backing.
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    snapshot.canonical.as_ptr(),
                                    core::ptr::addr_of_mut!(CANONICAL).cast::<u8>(),
                                    snapshot.canonical.len(),
                                );
                            }
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
        state.status.progress_current = true;
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
    // Publish terminal custody and return without another await. Management and
    // this worker share one executor, so Root cannot observe task_active=false
    // until this poll has returned and the executor has released registration.
    // The StaticCell remains one-use and the task pool's static bytes remain
    // reserved. Only authenticated retirement may consume the retained kernel;
    // task completion grants neither provider custody nor a new Play.
    slot.execution.replace(Some(execution));
    update(|state| state.status.task_active = false);
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
