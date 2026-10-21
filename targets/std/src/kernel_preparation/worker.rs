//! Finite private worker control for the initialized region service.
use conduit_composite::{
    ChildExecutionError, ChildTransportError, PreparedRegionKernel, RegionMemoryReceipt,
};
use conduit_kernel::{
    CanonicalValue, CordId, RemoteTerminalDisposition,
    scheduler::{RemoteIngressOutcome, SchedulerError, SchedulerStatus},
};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
#[path = "worker/run.rs"]
mod run;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkerError {
    Busy,
    InvalidLane,
    UnexpectedReply,
    Stopped,
    Poisoned,
    Panicked,
    NotInstalled,
    NotStarted,
    AlreadyStarted,
    AlreadyInstalled,
    StackMismatch,
    BufferTooSmall,
    Execution(ChildExecutionError),
    Transport(ChildTransportError),
    Scheduler(SchedulerError),
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkerReply {
    Installed(RegionMemoryReceipt),
    Started,
    Step(SchedulerStatus),
    Offer { sequence: Option<u64>, bytes: usize },
    Ingress(RemoteIngressOutcome),
    Delivered,
    Terminal(Option<RemoteTerminalDisposition>),
    Abnormal(Option<CanonicalValue>),
    Closed,
    Cancelled,
}
// The single prepared command slot owns the kernel without another allocation.
#[allow(clippy::large_enum_variant)]
enum Command {
    Install(PreparedRegionKernel),
    Start,
    Step,
    Offer(CordId),
    Admit { cord: CordId, sequence: u64 },
    Delivered { cord: CordId, sequence: u64 },
    Terminal(CordId),
    Abnormal(CordId),
    Close(CordId),
    CloseAbnormal(CordId, CanonicalValue),
    Cancel,
}
struct State {
    command: Option<Command>,
    reply: Option<Result<WorkerReply, WorkerError>>,
    buffer: Vec<u8>,
    initialized: bool,
    stopping: bool,
    exited: bool,
}
pub(crate) struct WorkerRetirement {
    pub(crate) kernel: Option<PreparedRegionKernel>,
    pub(crate) failure: Option<WorkerError>,
    pub(crate) cancellation: Option<Result<(), WorkerError>>,
}
struct Control {
    state: Mutex<State>,
    changed: Condvar,
    stack_bytes: u32,
}

/// At most one pending command or unread reply. The kernel remains private to
/// its OS thread; no channels, payload growth, or ambient worker pool is used.
pub(crate) struct InitializedRegionWorker {
    control: Arc<Control>,
    thread: Option<JoinHandle<WorkerRetirement>>,
}
impl InitializedRegionWorker {
    pub(crate) fn spawn(stack_bytes: u32, transfer_bytes: u32) -> Result<Self, String> {
        if !cfg!(target_os = "linux")
            || !(2 * 1024 * 1024..=16 * 1024 * 1024).contains(&stack_bytes)
            || !stack_bytes.is_multiple_of(4096)
            || !(1..=512 * 1024).contains(&transfer_bytes)
        {
            return Err(
                "worker stack/transfer capacity is outside the initialized Linux profile".into(),
            );
        }
        let control = Arc::new(Control {
            state: Mutex::new(State {
                command: None,
                reply: None,
                buffer: Vec::with_capacity(transfer_bytes as usize),
                initialized: false,
                stopping: false,
                exited: false,
            }),
            changed: Condvar::new(),
            stack_bytes,
        });
        let worker_control = Arc::clone(&control);
        let thread = std::thread::Builder::new()
            .stack_size(stack_bytes as usize)
            .spawn(move || run::serve(worker_control))
            .map_err(|e| format!("worker startup: {e}"))?;
        let worker = Self {
            control,
            thread: Some(thread),
        };
        {
            let mut state = worker
                .control
                .state
                .lock()
                .map_err(|_| "worker startup poisoned")?;
            while !state.initialized && !state.exited {
                state = worker
                    .control
                    .changed
                    .wait(state)
                    .map_err(|_| "worker startup poisoned")?;
            }
            if state.exited {
                return Err("worker exited during initialization".into());
            }
        }
        Ok(worker)
    }
    fn submit(&mut self, command: Command, payload: Option<&[u8]>) -> Result<(), WorkerError> {
        let mut state = self.control.state.try_lock().map_err(|e| match e {
            std::sync::TryLockError::WouldBlock => WorkerError::Busy,
            std::sync::TryLockError::Poisoned(_) => WorkerError::Poisoned,
        })?;
        self.commit_command(&mut state, command, payload)
    }
    fn submit_preparation(&self, command: Command) -> Result<(), WorkerError> {
        let mut state = self
            .control
            .state
            .lock()
            .map_err(|_| WorkerError::Poisoned)?;
        self.commit_command(&mut state, command, None)
    }
    fn commit_command(
        &self,
        state: &mut State,
        command: Command,
        payload: Option<&[u8]>,
    ) -> Result<(), WorkerError> {
        if state.stopping || state.exited {
            return Err(WorkerError::Stopped);
        }
        if state.command.is_some() || state.reply.is_some() {
            return Err(WorkerError::Busy);
        }
        if let Some(bytes) = payload {
            if bytes.len() > state.buffer.capacity() {
                return Err(WorkerError::BufferTooSmall);
            }
            state.buffer.clear();
            state.buffer.extend_from_slice(bytes);
        }
        state.command = Some(command);
        self.control.changed.notify_one();
        Ok(())
    }
    /// Copy any offered payload before releasing its reply slot. A too-small
    /// caller buffer leaves the same reply retained, without rerunning a Back.
    pub(crate) fn take_reply(
        &mut self,
        output: &mut [u8],
    ) -> Result<Option<Result<WorkerReply, WorkerError>>, WorkerError> {
        let mut state = self.control.state.try_lock().map_err(|e| match e {
            std::sync::TryLockError::WouldBlock => WorkerError::Busy,
            std::sync::TryLockError::Poisoned(_) => WorkerError::Poisoned,
        })?;
        if let Some(Ok(WorkerReply::Offer { bytes, .. })) = state.reply {
            if bytes > output.len() {
                return Err(WorkerError::BufferTooSmall);
            }
            output[..bytes].copy_from_slice(&state.buffer[..bytes]);
        }
        if let Some(reply) = state.reply.take() {
            return Ok(Some(reply));
        }
        if state.exited {
            return Err(WorkerError::Stopped);
        }
        Ok(None)
    }
    // Installation/start are preparation transitions. Runtime dispatch remains
    // nonblocking and lets the containing service submit all lanes first.
    fn wait_reply(&mut self) -> Result<WorkerReply, WorkerError> {
        let mut state = self
            .control
            .state
            .lock()
            .map_err(|_| WorkerError::Poisoned)?;
        while state.reply.is_none() && !state.exited {
            state = self
                .control
                .changed
                .wait(state)
                .map_err(|_| WorkerError::Poisoned)?;
        }
        state.reply.take().ok_or(WorkerError::Stopped)?
    }
    pub(crate) fn install(
        &mut self,
        kernel: PreparedRegionKernel,
    ) -> Result<RegionMemoryReceipt, WorkerError> {
        self.submit_preparation(Command::Install(kernel))?;
        match self.wait_reply()? {
            WorkerReply::Installed(memory) => Ok(memory),
            _ => Err(WorkerError::NotInstalled),
        }
    }
    pub(crate) fn start(&mut self) -> Result<(), WorkerError> {
        self.submit_preparation(Command::Start)?;
        match self.wait_reply()? {
            WorkerReply::Started => Ok(()),
            _ => Err(WorkerError::NotStarted),
        }
    }
    pub(crate) fn step(&mut self) -> Result<(), WorkerError> {
        self.submit(Command::Step, None)
    }
    pub(crate) fn offer(&mut self, cord: CordId) -> Result<(), WorkerError> {
        self.submit(Command::Offer(cord), None)
    }
    pub(crate) fn admit(
        &mut self,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<(), WorkerError> {
        self.submit(Command::Admit { cord, sequence }, Some(bytes))
    }
    pub(crate) fn delivered(&mut self, cord: CordId, sequence: u64) -> Result<(), WorkerError> {
        self.submit(Command::Delivered { cord, sequence }, None)
    }
    pub(crate) fn terminal(&mut self, cord: CordId) -> Result<(), WorkerError> {
        self.submit(Command::Terminal(cord), None)
    }
    pub(crate) fn abnormal(&mut self, cord: CordId) -> Result<(), WorkerError> {
        self.submit(Command::Abnormal(cord), None)
    }
    pub(crate) fn close(&mut self, cord: CordId) -> Result<(), WorkerError> {
        self.submit(Command::Close(cord), None)
    }
    pub(crate) fn close_abnormal(
        &mut self,
        cord: CordId,
        value: CanonicalValue,
    ) -> Result<(), WorkerError> {
        self.submit(Command::CloseAbnormal(cord, value), None)
    }
    pub(crate) fn cancel(&mut self) -> Result<(), WorkerError> {
        self.submit(Command::Cancel, None)
    }
    pub(crate) fn is_alive(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }
    pub(crate) fn stack_bytes(&self) -> u32 {
        self.control.stack_bytes
    }
    /// Managed control data and payload capacity only; Arc/thread/allocator/OS
    /// bookkeeping belongs to the initialized Host service profile.
    pub(crate) fn control_backing_bytes(&self) -> Result<usize, WorkerError> {
        let state = self
            .control
            .state
            .lock()
            .map_err(|_| WorkerError::Poisoned)?;
        Ok(
            core::mem::size_of::<Self>()
                + core::mem::size_of::<Control>()
                + state.buffer.capacity(),
        )
    }
    pub(crate) fn shutdown(mut self) -> Result<WorkerRetirement, WorkerError> {
        self.stop_and_join()
    }
    fn stop_and_join(&mut self) -> Result<WorkerRetirement, WorkerError> {
        // A poisoned control lock still owns the stop flag. Preserve cleanup
        // after a Back panic without treating the failed worker as successful.
        let mut state = self.control.state.lock().unwrap_or_else(|e| e.into_inner());
        state.stopping = true;
        self.control.changed.notify_one();
        drop(state);
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| WorkerError::Panicked)
        } else {
            Ok(WorkerRetirement {
                kernel: None,
                failure: None,
                cancellation: None,
            })
        }
    }
}
impl Drop for InitializedRegionWorker {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}
