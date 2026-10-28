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
#[path = "native_task_terminal.rs"]
mod terminal;

type NativeProviders = crate::native_worker::RetiredProviders<
    crate::native_root::DevicePulse<'static>,
    crate::native_wait::EspWait,
>;
/// Settled original execution material and publication custody, never a grant
/// to reset their finite budgets or reuse their old execution scope.
pub struct Retired {
    pub execution: crate::native_worker::RetiredExecution<
        crate::native_root::DevicePulse<'static>,
        crate::native_wait::EspWait,
    >,
    #[cfg(feature = "native-face-root")]
    pub publisher: crate::native_face::Publisher,
}
struct Slot {
    execution: RefCell<Option<Box<DeviceExecution>>>,
    #[cfg(feature = "native-face-root")]
    publisher: RefCell<Option<crate::native_face::Publisher>>,
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
// ESP32 RTC-slow data RAM is unused by this awake profile.
// This is volatile scratch, never retained state across reset or DMA backing.
#[cfg_attr(
    all(target_os = "none", feature = "native-face-root"),
    esp_hal::ram(unstable(rtc_slow))
)]
#[cfg_attr(
    all(target_os = "none", not(feature = "native-face-root")),
    esp_hal::ram(reclaimed)
)]
#[cfg(not(feature = "local-appliance"))]
static mut CANONICAL: core::mem::MaybeUninit<[u8; PROJECTION_BYTES]> =
    core::mem::MaybeUninit::uninit();
const EMPTY_STATUS: Status = Status {
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
};
static STATE: Mutex<CriticalSectionRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State {
    cancel_requested: false,
    status: EMPTY_STATUS,
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
            let ready = state.status.native_live
                && state.status.native_in_flight == Some([0, 0])
                && state.status.native_lifecycles == Some(["issued", "issued"])
                && state.status.progress_current
                && state.status.steps == 0
                && state.status.publication_sequence == 0;
            let cancelled = state.cancel_requested;
            crate::native_execution_gate::accept_start(&mut state.status.phase, cancelled, ready)
        })
    }
    pub fn release(&self) -> Result<(), &'static str> {
        STATE.lock(|state| {
            let mut state = state.borrow_mut();
            let cancelled = state.cancel_requested;
            crate::native_execution_gate::release(&mut state.status.phase, cancelled)?;
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
    /// Compatibility transfer for callers which retain only physical providers.
    /// Other terminal material is dropped; no replacement publication is issued.
    pub fn retire(&self) -> Result<NativeProviders, &'static str> {
        Ok(self.take_retired()?.execution.providers)
    }
    /// Consume the exclusive task handle while returning all actual terminal
    /// custody. The returned handle alone may register separately admitted next
    /// owners; this operation grants no new Face, kernel, Body or Play scope.
    pub fn retire_with_publication(self) -> Result<(Self, Retired), (Self, &'static str)> {
        match self.take_retired() {
            Ok(retired) => Ok((self, retired)),
            Err(reason) => Err((self, reason)),
        }
    }
    fn take_retired(&self) -> Result<Retired, &'static str> {
        let status = self.status();
        if !terminal::retirement_ready(
            status.phase,
            status.task_active,
            status.progress_current,
            status.failure.is_some(),
            status.cleanup_failure,
            status.native_lifecycles,
            status.native_in_flight,
        ) {
            return Err("worker-not-quiesced");
        }
        // Hold every slot borrow and preflight publication custody before the
        // execution's consuming retirement. Refusal leaves all owners in place.
        let mut execution = self
            .slot
            .execution
            .try_borrow_mut()
            .map_err(|_| "worker-execution-busy")?;
        #[cfg(feature = "native-face-root")]
        let mut publisher = self
            .slot
            .publisher
            .try_borrow_mut()
            .map_err(|_| "worker-publisher-busy")?;
        #[cfg(feature = "native-face-root")]
        if publisher.is_none() {
            return Err("worker-publisher-unavailable");
        }
        let execution = DeviceExecution::retire_with_material_retained(&mut execution)?;
        let retired = Retired {
            execution,
            #[cfg(feature = "native-face-root")]
            publisher: publisher.take().expect("exclusively preflighted publisher"),
        };
        update(|state| {
            state.status.phase = "retired";
            state.status.context_bytes = 0;
        });
        Ok(retired)
    }
    /// Reuse only this exclusively held slot after actual kernel/provider
    /// retirement. The caller supplies a separately admitted fresh execution;
    /// this does not rearm the old scheduler or grant a new Body Play. Consuming
    /// the handle prevents a prior registration from commanding its successor.
    pub fn prepare_next(
        self,
        execution: &mut Option<Box<DeviceExecution>>,
        spawner: Spawner,
        #[cfg(feature = "native-face-root")] publisher: &mut Option<crate::native_face::Publisher>,
    ) -> Result<Self, (Self, &'static str)> {
        let status = self.status();
        if status.phase != "retired" || status.task_active || self.slot.execution.borrow().is_some()
        {
            return Err((self, "worker-slot-not-retired"));
        }
        #[cfg(feature = "native-face-root")]
        if self.slot.publisher.borrow().is_some() || publisher.is_none() {
            return Err((self, "worker-publisher-retained"));
        }
        if execution.is_none() {
            return Err((self, "execution-context-unavailable"));
        }
        match register(
            self.slot,
            execution,
            spawner,
            #[cfg(feature = "native-face-root")]
            publisher,
        ) {
            Ok(next) => Ok(next),
            Err(reason) => {
                // The failed registration returned the incoming execution and
                // never polled it. Keep the exclusive slot retryable by its owner.
                update(|state| state.status.phase = "retired");
                Err((self, reason))
            }
        }
    }
    /// This appliance delivers canonical readings through its actual Face and
    /// signed history consumers; it does not also retain the legacy UART copy.
    #[cfg(feature = "local-appliance")]
    pub fn copy_publication(&self, _: &mut [u8]) -> Result<Option<Status>, &'static str> {
        Err("use-local-face-or-signed-history")
    }
    /// Copy under the short critical section; parsing/Mask work happens outside.
    #[cfg(not(feature = "local-appliance"))]
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
    #[cfg(feature = "native-face-root")] publisher: crate::native_face::Publisher,
) -> Result<Handle, &'static str> {
    if execution.is_none() {
        return Err("execution-context-unavailable");
    }
    let slot = SLOT
        .try_init_with(|| Slot {
            execution: RefCell::new(None),
            #[cfg(feature = "native-face-root")]
            publisher: RefCell::new(None),
        })
        .ok_or("worker-slot-already-used")?;
    #[cfg(feature = "native-face-root")]
    let mut publisher = Some(publisher);
    register(
        slot,
        execution,
        spawner,
        #[cfg(feature = "native-face-root")]
        &mut publisher,
    )
}
fn register(
    slot: &'static Slot,
    execution: &mut Option<Box<DeviceExecution>>,
    spawner: Spawner,
    #[cfg(feature = "native-face-root")] publisher: &mut Option<crate::native_face::Publisher>,
) -> Result<Handle, &'static str> {
    // Root and this task share one executor. The old future has returned before
    // retirement is observable. Discard prior signals only at this new exclusive
    // registration; no old command or ready acknowledgement crosses the boundary.
    COMMAND.reset();
    READY.reset();
    slot.execution.replace(execution.take());
    #[cfg(feature = "native-face-root")]
    slot.publisher.replace(publisher.take());
    update(|state| {
        state.cancel_requested = false;
        state.status = EMPTY_STATUS;
        state.status.phase = "registering";
        state.status.task_active = true;
        state.status.context_bytes = core::mem::size_of::<DeviceExecution>();
        state.status.held_future_bytes = core::mem::size_of_val(&held(slot));
    });
    match worker(slot) {
        Ok(token) => {
            spawner.spawn(token);
            Ok(Handle { slot })
        }
        Err(_) => {
            *execution = slot.execution.borrow_mut().take();
            #[cfg(feature = "native-face-root")]
            {
                *publisher = slot.publisher.borrow_mut().take();
            }
            update(|state| {
                state.status.phase = "registration-refused";
                state.status.task_active = false;
            });
            Err("worker-task-pool-unavailable")
        }
    }
}
// Exact stable expansion from pinned embassy-executor0.10.0 / macros0.8.0,
// with only the initialized pool's placement changed. CPU0 awake data only;
// neither DMA, second-core access nor persistence across sleep is admitted.
fn worker(
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
    let Some(mut execution) = slot.execution.borrow_mut().take() else {
        update(|state| {
            state.status.phase = "ownership-refused";
            state.status.task_active = false;
        });
        return;
    };
    #[cfg(feature = "native-face-root")]
    let mut publisher = slot
        .publisher
        .borrow_mut()
        .take()
        .expect("prepared Face publisher");
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
                        let committed_ms = embassy_time::Instant::now().as_millis();
                        #[cfg(feature = "native-face-root")]
                        publisher.publish(&snapshot, committed_ms);
                        update(|state| {
                            state.status.source_sequence = snapshot.sequence;
                            if snapshot.canonical.len() > PROJECTION_BYTES {
                                state.status.projection_refused = true;
                                return;
                            }
                            // SAFETY: bounded above and serialized by STATE;
                            // the kernel snapshot cannot alias this private backing.
                            #[cfg(not(feature = "local-appliance"))]
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
                            state.status.committed_millis = Some(committed_ms);
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
    // Static slot and task-pool backing remain reserved. Only exact retirement
    // may consume the retained kernel; a later registration consumes the exclusive
    // handle and independently prepared owners. Completion grants no new Play.
    // Publication Drop used to withdraw HTTP at this point. Retain the actual
    // publisher for consuming retirement, but preserve that terminal withdrawal.
    #[cfg(feature = "native-face-root")]
    {
        publisher.withdraw_http();
        slot.publisher.replace(Some(publisher));
    }
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
