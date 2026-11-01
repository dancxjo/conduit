//! Fixed one-slot native IO worker. No Region kernel, channel or payload growth.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, TryLockError,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
pub const NATIVE_SOURCE_WORKER_PROFILE: &str = "conduit.std/linux-native-source-one-slot@1";
const CONTROL_ALLOWANCE: u32 = 64 * 1024;
pub const MAXIMUM_REQUEST_BYTES: usize = 1536;
pub const MAXIMUM_RESULT_BYTES: usize = 16 * 1024;

/// Explicit cooperative Linux profile. Requested stack size is not an observed
/// OS mapping size. The worker separately checks pthread stack+guard at ready.
/// The fixed control allowance bounds this profile's Rust control/coordination
/// backing; input/result buffers are charged separately by exact length. This
/// is not a receipt for whole-process RAM, allocator arenas or runtime TLS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeWorkerBudget {
    pub requested_stack_bytes: u32,
    pub maximum_stack_mapping_bytes: u32,
    pub capture_timeout: Duration,
    pub cancellation_timeout: Duration,
}
impl NativeWorkerBudget {
    pub fn backing_bytes(self, input: u32, output: u32) -> Result<u32, WorkerFailure> {
        if !cfg!(target_os = "linux")
            || !(2 * 1024 * 1024..=16 * 1024 * 1024).contains(&self.requested_stack_bytes)
            || !self.requested_stack_bytes.is_multiple_of(4096)
            || self.maximum_stack_mapping_bytes < self.requested_stack_bytes
            || self.maximum_stack_mapping_bytes > self.requested_stack_bytes + 64 * 1024
            || !self.maximum_stack_mapping_bytes.is_multiple_of(4096)
            || !(1..=MAXIMUM_REQUEST_BYTES as u32).contains(&input)
            || !(1..=MAXIMUM_RESULT_BYTES as u32).contains(&output)
            || self.capture_timeout.is_zero()
            || self.capture_timeout > Duration::from_secs(45)
            || self.cancellation_timeout.is_zero()
            || self.cancellation_timeout > Duration::from_secs(5)
        {
            return Err(WorkerFailure::Profile);
        }
        self.maximum_stack_mapping_bytes
            .checked_add(CONTROL_ALLOWANCE)
            .and_then(|v| v.checked_add(input))
            .and_then(|v| v.checked_add(output))
            .ok_or(WorkerFailure::Profile)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerFailure {
    Profile,
    Busy,
    Lost,
    Provider,
    InvalidOutput,
    Cancelled,
    TimedOut,
    Poisoned,
    Sequence,
}
/// Untrusted source clocks remain provider metadata, never this deadline.
pub struct NativeReadContext<'a> {
    cancelled: &'a AtomicBool,
    deadline: Instant,
}
impl NativeReadContext<'_> {
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    pub fn should_stop(&self) -> bool {
        self.cancelled() || self.remaining().is_zero()
    }
}
/// # Safety
/// The actual initialized native owner must match the independently retained
/// Source definition. Keep its continuity across child Plays, honor the supplied
/// deadline and cancellation bound, allocate nothing during read, retain no
/// borrowed buffers, and do not unwind. No Body/MCU identity follows from IO.
pub unsafe trait NativeSourceWorkerProvider: Send {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure>;
}
#[derive(Clone, Copy, Debug)]
pub struct WorkerBackingObservation {
    pub stack_bytes: usize,
    pub guard_bytes: usize,
    pub charged_bytes: u32,
}
#[derive(Clone, Copy)]
struct Request {
    generation: u64,
    length: usize,
    deadline: Instant,
}
#[derive(Clone, Copy)]
struct Reply {
    generation: u64,
    result: Result<usize, WorkerFailure>,
}
struct State {
    provider: Option<Box<dyn NativeSourceWorkerProvider>>,
    input: Box<[u8]>,
    output: Box<[u8]>,
    request: Option<Request>,
    reply: Option<Reply>,
    ready: Option<Result<WorkerBackingObservation, WorkerFailure>>,
    stopping: bool,
    running: bool,
    next: u64,
}
struct Control {
    state: Mutex<State>,
    changed: Condvar,
    cancelled: AtomicBool,
    exited: AtomicBool,
    cancellation_overdue: AtomicBool,
}
pub(super) struct NativeWorker {
    control: Arc<Control>,
    thread: Option<JoinHandle<()>>,
    budget: NativeWorkerBudget,
    observation: Option<WorkerBackingObservation>,
}
impl NativeWorker {
    /// Caller must already own the actual ledger backing reservation. This
    /// function is deliberately private to that admitted native installer.
    pub(super) fn spawn(
        provider: Box<dyn NativeSourceWorkerProvider>,
        budget: NativeWorkerBudget,
        input: u32,
        output: u32,
    ) -> Result<Self, (Box<dyn NativeSourceWorkerProvider>, WorkerFailure)> {
        let charged = match budget.backing_bytes(input, output) {
            Ok(v) => v,
            Err(e) => return Err((provider, e)),
        };
        let control = Arc::new(Control {
            state: Mutex::new(State {
                provider: Some(provider),
                input: vec![0; input as usize].into_boxed_slice(),
                output: vec![0; output as usize].into_boxed_slice(),
                request: None,
                reply: None,
                ready: None,
                stopping: false,
                running: false,
                next: 1,
            }),
            changed: Condvar::new(),
            cancelled: AtomicBool::new(false),
            exited: AtomicBool::new(false),
            cancellation_overdue: AtomicBool::new(false),
        });
        let child = Arc::clone(&control);
        let thread = match std::thread::Builder::new()
            .stack_size(budget.requested_stack_bytes as usize)
            .spawn(move || serve(child, budget, charged))
        {
            Ok(v) => v,
            Err(_) => {
                return Err((
                    control.state.lock().unwrap().provider.take().unwrap(),
                    WorkerFailure::Lost,
                ))
            }
        };
        let mut worker = Self {
            control,
            thread: Some(thread),
            budget,
            observation: None,
        };
        let ready = {
            let mut state = worker.control.state.lock().unwrap();
            while state.ready.is_none() && !worker.control.exited.load(Ordering::Acquire) {
                state = worker.control.changed.wait(state).unwrap();
            }
            state.ready.unwrap_or(Err(WorkerFailure::Lost))
        };
        if let Err(error) = ready {
            worker.stop();
            worker.thread.take().unwrap().join().ok();
            let provider = worker
                .control
                .state
                .lock()
                .unwrap()
                .provider
                .take()
                .expect("startup never invokes provider");
            return Err((provider, error));
        }
        worker.observation = ready.ok();
        Ok(worker)
    }
    pub(super) fn ready_for_play(&self) -> bool {
        self.control.state.try_lock().is_ok_and(|state| {
            !state.stopping
                && !state.running
                && state.request.is_none()
                && state.reply.is_none()
                && !self.control.exited.load(Ordering::Acquire)
        })
    }
    pub(super) fn submit(&mut self, input: &[u8]) -> Result<u64, WorkerFailure> {
        let mut state = self.control.state.try_lock().map_err(lock_error)?;
        if state.stopping || self.control.exited.load(Ordering::Acquire) {
            return Err(WorkerFailure::Lost);
        }
        if state.running || state.request.is_some() || state.reply.is_some() {
            return Err(WorkerFailure::Busy);
        }
        if input.len() > state.input.len() {
            return Err(WorkerFailure::Profile);
        }
        let generation = state.next;
        state.next = state.next.checked_add(1).ok_or(WorkerFailure::Sequence)?;
        state.input[..input.len()].copy_from_slice(input);
        self.control.cancelled.store(false, Ordering::Release);
        state.request = Some(Request {
            generation,
            length: input.len(),
            deadline: Instant::now() + self.budget.capture_timeout,
        });
        self.control.changed.notify_one();
        Ok(generation)
    }
    pub(super) fn poll(
        &mut self,
        generation: u64,
        output: &mut [u8],
    ) -> Result<Option<Result<usize, WorkerFailure>>, WorkerFailure> {
        let mut state = match self.control.state.try_lock() {
            Ok(v) => v,
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(_) => return Err(WorkerFailure::Poisoned),
        };
        let Some(reply) = state.reply else {
            return if self.control.exited.load(Ordering::Acquire) {
                Err(WorkerFailure::Lost)
            } else {
                Ok(None)
            };
        };
        if reply.result == Err(WorkerFailure::Lost) && !self.control.exited.load(Ordering::Acquire)
        {
            return Ok(None);
        }
        if reply.generation != generation {
            return Err(WorkerFailure::Sequence);
        }
        if let Ok(length) = reply.result {
            if length > output.len() {
                return Err(WorkerFailure::InvalidOutput);
            }
            output[..length].copy_from_slice(&state.output[..length]);
        }
        state.reply = None;
        Ok(Some(reply.result))
    }
    pub(super) fn cancel(&self) {
        self.control.cancelled.store(true, Ordering::Release);
        self.control.changed.notify_all();
    }
    fn stop(&self) {
        self.cancel();
        let mut state = self.control.state.lock().unwrap();
        state.stopping = true;
        self.control.changed.notify_all();
    }
    pub(super) fn is_quiescent(&self) -> bool {
        self.control
            .state
            .try_lock()
            .is_ok_and(|s| !s.running && s.request.is_none())
    }
    /// Busy leaves both thread and actual provider owned. No fake successful
    /// retirement is returned for a stubborn provider, even past its deadline.
    pub(super) fn retire(
        &mut self,
    ) -> Result<Option<Box<dyn NativeSourceWorkerProvider>>, WorkerFailure> {
        self.stop();
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Err(WorkerFailure::Busy);
        }
        self.thread
            .take()
            .unwrap()
            .join()
            .map_err(|_| WorkerFailure::Lost)?;
        Ok(self
            .control
            .state
            .lock()
            .map_err(|_| WorkerFailure::Poisoned)?
            .provider
            .take())
    }
    pub(super) fn observation(&self) -> WorkerBackingObservation {
        self.observation.expect("only ready worker is exposed")
    }
    pub(super) fn cancellation_overdue(&self) -> bool {
        self.control.cancellation_overdue.load(Ordering::Acquire)
    }
    pub(super) fn mark_cancellation_overdue(&self) {
        self.control
            .cancellation_overdue
            .store(true, Ordering::Release);
    }
    pub(super) fn cancellation_timeout(&self) -> Duration {
        self.budget.cancellation_timeout
    }
}
impl Drop for NativeWorker {
    fn drop(&mut self) {
        // Root destruction is terminal, not permission to detach a live physical
        // session. A violating cooperative provider can prevent shutdown; it cannot
        // make its backing reusable while still running.
        if self.thread.is_some() {
            self.stop();
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
}
fn lock_error<T>(error: TryLockError<T>) -> WorkerFailure {
    match error {
        TryLockError::WouldBlock => WorkerFailure::Busy,
        TryLockError::Poisoned(_) => WorkerFailure::Poisoned,
    }
}
fn serve(control: Arc<Control>, budget: NativeWorkerBudget, charged: u32) {
    let observed = stack_observation(charged).and_then(|value| {
        if value
            .stack_bytes
            .checked_add(value.guard_bytes)
            .is_some_and(|bytes| bytes <= budget.maximum_stack_mapping_bytes as usize)
            && std::mem::size_of::<Control>() + std::mem::size_of::<State>() + 1024
                < CONTROL_ALLOWANCE as usize
        {
            Ok(value)
        } else {
            Err(WorkerFailure::Profile)
        }
    });
    {
        let mut state = control.state.lock().unwrap();
        state.ready = Some(observed);
        control.changed.notify_all();
    }
    if observed.is_err() {
        control.exited.store(true, Ordering::Release);
        return;
    }
    let mut input = [0; MAXIMUM_REQUEST_BYTES];
    let mut output = [0; MAXIMUM_RESULT_BYTES];
    loop {
        let (request, mut provider, capacity) = {
            let mut state = control.state.lock().unwrap();
            while state.request.is_none() && !state.stopping {
                state = control.changed.wait(state).unwrap();
            }
            if state.stopping {
                break;
            }
            let request = state.request.take().unwrap();
            input[..request.length].copy_from_slice(&state.input[..request.length]);
            state.running = true;
            (request, state.provider.take().unwrap(), state.output.len())
        };
        let context = NativeReadContext {
            cancelled: &control.cancelled,
            deadline: request.deadline,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            provider.read(&input[..request.length], &mut output[..capacity], &context)
        }));
        let mut state = control.state.lock().unwrap();
        state.running = false;
        let result = match result {
            Ok(result) => {
                state.provider = Some(provider);
                result
            }
            Err(_) => {
                state.stopping = true;
                Err(WorkerFailure::Lost)
            }
        };
        let result = if control.cancelled.load(Ordering::Acquire) {
            Err(WorkerFailure::Cancelled)
        } else if Instant::now() >= request.deadline {
            Err(WorkerFailure::TimedOut)
        } else {
            result.and_then(|length| {
                if length <= capacity {
                    Ok(length)
                } else {
                    Err(WorkerFailure::InvalidOutput)
                }
            })
        };
        if let Ok(length) = result {
            state.output[..length].copy_from_slice(&output[..length]);
        }
        state.reply = Some(Reply {
            generation: request.generation,
            result,
        });
        control.changed.notify_all();
        if state.stopping {
            break;
        }
    }
    control.exited.store(true, Ordering::Release);
    control.changed.notify_all();
}
#[cfg(target_os = "linux")]
fn stack_observation(charged: u32) -> Result<WorkerBackingObservation, WorkerFailure> {
    let mut attributes = std::mem::MaybeUninit::<libc::pthread_attr_t>::uninit();
    // SAFETY: pthread writes this thread's initialized attribute object. Its
    // observed mapping is read-only metadata and destroyed exactly once.
    unsafe {
        if libc::pthread_getattr_np(libc::pthread_self(), attributes.as_mut_ptr()) != 0 {
            return Err(WorkerFailure::Profile);
        }
        let mut attributes = attributes.assume_init();
        let mut address = std::ptr::null_mut();
        let mut bytes = 0;
        let mut guard = 0;
        let valid = libc::pthread_attr_getstack(&attributes, &mut address, &mut bytes) == 0
            && libc::pthread_attr_getguardsize(&attributes, &mut guard) == 0;
        libc::pthread_attr_destroy(&mut attributes);
        if !valid || address.is_null() || bytes == 0 {
            return Err(WorkerFailure::Profile);
        }
        Ok(WorkerBackingObservation {
            stack_bytes: bytes,
            guard_bytes: guard,
            charged_bytes: charged,
        })
    }
}
#[cfg(not(target_os = "linux"))]
fn stack_observation(_: u32) -> Result<WorkerBackingObservation, WorkerFailure> {
    Err(WorkerFailure::Profile)
}
