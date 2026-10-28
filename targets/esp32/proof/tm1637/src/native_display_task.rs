//! Single-executor custody for the actual display context. No execution before
//! attributed Release; an unbound input Fore sleeps until cancellation.
use super::{
    execution::{Execution, Exit},
    DisplayPins,
};
use alloc::boxed::Box;
use core::cell::RefCell;
use embassy_sync::{
    blocking_mutex::{raw::CriticalSectionRawMutex, Mutex},
    signal::Signal,
};
use static_cell::StaticCell;
type Device = Execution<DisplayPins<'static>>;
struct Slot {
    execution: RefCell<Option<Box<Device>>>,
    #[cfg(feature = "display-physical-check")]
    fore: RefCell<Option<super::check_task::FaceFore>>,
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
    pub progress_current: bool,
    pub context_bytes: usize,
    pub held_future_bytes: usize,
    pub steps: u64,
    pub native_calls: u64,
    pub software_calls: u64,
    pub failure: Option<&'static str>,
    pub cleanup_failure: bool,
    pub native_custody: bool,
    pub native_lifecycle: &'static str,
    pub native_in_flight: u16,
}
struct State {
    cancelled: bool,
    status: Status,
}
// CPU-only lifecycle/status custody; no DMA, GPIO timing, or deep-sleep
// persistence. Keep its finite backing in the admitted RTC slow RAM alongside
// the sampler/Mask status, preserving ordinary SRAM for the CPU stack.
#[cfg_attr(
    all(target_os = "none", feature = "local-appliance"),
    esp_hal::ram(unstable(rtc_slow))
)]
static STATE: Mutex<CriticalSectionRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State {
    cancelled: false,
    status: Status {
        phase: "unregistered",
        task_active: false,
        progress_current: true,
        context_bytes: 0,
        held_future_bytes: 0,
        steps: 0,
        native_calls: 0,
        software_calls: 0,
        failure: None,
        cleanup_failure: false,
        native_custody: false,
        native_lifecycle: "unavailable",
        native_in_flight: 0,
    },
}));
fn update(f: impl FnOnce(&mut State)) {
    STATE.lock(|s| f(&mut s.borrow_mut()));
}
pub struct Handle {
    slot: &'static Slot,
}
impl Handle {
    pub fn status(&self) -> Status {
        STATE.lock(|s| s.borrow().status)
    }
    pub async fn wait_ready(&self) {
        READY.wait().await;
    }
    pub fn accept_start(&self) -> Result<(), &'static str> {
        STATE.lock(|s| {
            let mut s = s.borrow_mut();
            let cancelled = s.cancelled;
            let ready = s.status.native_custody
                && s.status.native_lifecycle == "issued"
                && s.status.native_in_flight == 0
                && s.status.progress_current
                && s.status.steps == 0;
            crate::native_execution_gate::accept_start(&mut s.status.phase, cancelled, ready)
        })
    }
    pub fn release(&self) -> Result<(), &'static str> {
        STATE.lock(|s| {
            let mut s = s.borrow_mut();
            let cancelled = s.cancelled;
            crate::native_execution_gate::release(&mut s.status.phase, cancelled)?;
            COMMAND.signal(Command::Release);
            Ok(())
        })
    }
    pub fn cancel(&self) -> Result<(), &'static str> {
        STATE.lock(|s| {
            let mut s = s.borrow_mut();
            if matches!(
                s.status.phase,
                "retired" | "failed" | "drained" | "cleanup-refused"
            ) {
                return Err("display-task-terminal");
            }
            if s.status.phase == "quiesced" {
                return Ok(());
            }
            s.cancelled = true;
            s.status.phase = "cancellation-requested";
            COMMAND.signal(Command::Cancel);
            Ok(())
        })
    }
    pub fn retire(
        &self,
    ) -> Result<crate::native_retirement::RetiredDisplay<DisplayPins<'static>>, &'static str> {
        let status = self.status();
        if status.task_active
            || status.cleanup_failure
            || !matches!(status.phase, "quiesced" | "drained" | "failed")
        {
            return Err("display-task-not-quiescent");
        }
        let returned = Execution::retire_retained(&mut self.slot.execution.borrow_mut())?;
        update(|s| {
            s.status.phase = "retired";
            s.status.native_custody = false;
        });
        Ok(returned)
    }
}
pub fn prepare(
    execution: &mut Option<Box<Device>>,
    spawner: embassy_executor::Spawner,
    #[cfg(feature = "display-physical-check")] fore: Option<super::check_task::FaceFore>,
) -> Result<Handle, &'static str> {
    if execution.is_none() {
        return Err("display-context-unavailable");
    }
    let slot = SLOT
        .try_init_with(|| Slot {
            execution: RefCell::new(None),
            #[cfg(feature = "display-physical-check")]
            fore: RefCell::new(fore),
        })
        .ok_or("display-task-slot-used")?;
    slot.execution.replace(execution.take());
    update(|s| {
        s.status.phase = "registering";
        s.status.task_active = true;
        s.status.context_bytes = core::mem::size_of::<Device>();
        s.status.held_future_bytes = core::mem::size_of_val(&held(slot));
    });
    match worker(slot) {
        Ok(token) => {
            spawner.spawn(token);
            Ok(Handle { slot })
        }
        Err(_) => {
            *execution = slot.execution.borrow_mut().take();
            update(|s| {
                s.status.phase = "registration-refused";
                s.status.task_active = false;
            });
            Err("display-task-pool-unavailable")
        }
    }
}
#[embassy_executor::task]
async fn worker(slot: &'static Slot) {
    held(slot).await;
}
fn native(status: &mut Status, execution: &Device) {
    let inspection = execution.inspection();
    status.native_custody = true;
    status.native_lifecycle = match inspection.lifecycle {
        conduit_core::CapabilityLifecycle::Issued => "issued",
        conduit_core::CapabilityLifecycle::Revoked => "revoked",
        conduit_core::CapabilityLifecycle::Exhausted => "exhausted",
    };
    status.native_in_flight = inspection.in_flight_operations;
}
async fn held(slot: &'static Slot) {
    let Some(mut execution) = slot.execution.borrow_mut().take() else {
        update(|s| {
            s.status.phase = "ownership-refused";
            s.status.task_active = false;
        });
        return;
    };
    update(|s| {
        s.status.phase = if s.cancelled {
            "cancellation-requested"
        } else {
            "ready-held"
        };
        native(&mut s.status, &execution);
    });
    READY.signal(());
    let outcome = match COMMAND.wait().await {
        Command::Cancel => execution.cancel_held().expect("unpolled display context"),
        Command::Release => {
            update(|s| {
                s.status.phase = "running";
                s.status.progress_current = false;
            });
            #[cfg(feature = "display-physical-check")]
            let fore = slot.fore.borrow_mut().take();
            #[cfg(feature = "display-physical-check")]
            if let Some(mut fore) = fore {
                execution
                    .run_with_fore(
                        async {
                            let _ = COMMAND.wait().await;
                        },
                        &mut fore,
                        super::CONSTRUCTION_FRAMES,
                    )
                    .await
            } else {
                execution
                    .run_with_idle(
                        async {
                            let _ = COMMAND.wait().await;
                        },
                        core::future::pending::<()>,
                    )
                    .await
            }
            #[cfg(not(feature = "display-physical-check"))]
            execution
                .run_with_idle(
                    async {
                        let _ = COMMAND.wait().await;
                    },
                    || {
                        update(|s| s.status.phase = "waiting-input");
                        core::future::pending::<()>()
                    },
                )
                .await
        }
    };
    update(|s| {
        s.status.phase = if outcome.cleanup.is_err() {
            "cleanup-refused"
        } else {
            match outcome.exit {
                Exit::Drained => "drained",
                Exit::Cancelled => "quiesced",
                Exit::Failed(_) => "failed",
            }
        };
        s.status.progress_current = true;
        s.status.steps = outcome.progress.steps;
        s.status.native_calls = outcome.progress.native_calls;
        s.status.software_calls = outcome.progress.software_calls;
        s.status.cleanup_failure = outcome.cleanup.is_err();
        s.status.failure = match outcome.exit {
            Exit::Failed(reason) => Some(reason.code.as_str()),
            _ => None,
        };
        native(&mut s.status, &execution);
    });
    slot.execution.replace(Some(execution));
    // Same executor as Root: terminal custody is visible after this poll returns.
    update(|s| s.status.task_active = false);
}

#[cfg(feature = "display-physical-check")]
pub fn report_boundary(
    phase: &'static str,
    progress: super::execution::Progress,
    inspection: &conduit_core::CapabilityInspection,
) {
    update(|s| {
        s.status.phase = phase;
        s.status.progress_current = true;
        s.status.steps = progress.steps;
        s.status.native_calls = progress.native_calls;
        s.status.software_calls = progress.software_calls;
        s.status.native_in_flight = inspection.in_flight_operations;
        s.status.native_lifecycle = match inspection.lifecycle {
            conduit_core::CapabilityLifecycle::Issued => "issued",
            conduit_core::CapabilityLifecycle::Revoked => "revoked",
            conduit_core::CapabilityLifecycle::Exhausted => "exhausted",
        };
    });
}
#[cfg(feature = "display-physical-check")]
impl Handle {
    pub fn stage_terminal_evidence(&self) -> Result<(), &'static str> {
        if !self.status().task_active {
            if let Some(execution) = self.slot.execution.borrow().as_deref() {
                execution.stage_evidence()?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "display-physical-check")]
pub fn mark_running() {
    update(|s| {
        if !s.cancelled {
            s.status.phase = "running";
            s.status.progress_current = false;
        }
    });
}
