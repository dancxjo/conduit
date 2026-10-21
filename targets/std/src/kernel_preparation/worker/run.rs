//! Worker-owned kernel advancement; all responses are finite typed values.
use super::*;
pub(super) fn serve(control: Arc<Control>) -> WorkerRetirement {
    let mut kernel: Option<PreparedRegionKernel> = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        serve_inner(Arc::clone(&control), &mut kernel)
    }));
    let cancellation = kernel.as_mut().map(|kernel| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| kernel.cancel()))
            .map_err(|_| WorkerError::Panicked)
            .and_then(|result| result.map_err(WorkerError::Scheduler))
    });
    let mut state = control.state.lock().unwrap_or_else(|e| e.into_inner());
    state.exited = true;
    let failure = if result.is_err() {
        Some(WorkerError::Panicked)
    } else if control.state.is_poisoned() {
        Some(WorkerError::Poisoned)
    } else {
        cancellation.and_then(Result::err)
    };
    if let Some(error) = failure {
        state.reply = Some(Err(error));
    }
    control.changed.notify_all();
    WorkerRetirement {
        kernel,
        failure,
        cancellation,
    }
}
fn serve_inner(control: Arc<Control>, kernel: &mut Option<PreparedRegionKernel>) {
    let mut started = false;
    let mut state = match control.state.lock() {
        Ok(state) => state,
        Err(_) => return,
    };
    state.initialized = true;
    control.changed.notify_one();
    loop {
        while state.command.is_none() && !state.stopping {
            state = match control.changed.wait(state) {
                Ok(state) => state,
                Err(_) => return,
            };
        }
        if state.stopping {
            break;
        }
        let Some(command) = state.command.take() else {
            continue;
        };
        state.reply = Some(execute(
            command,
            kernel,
            &mut started,
            &mut state.buffer,
            control.stack_bytes,
        ));
        control.changed.notify_one();
    }
}
fn execute(
    command: Command,
    kernel: &mut Option<PreparedRegionKernel>,
    started: &mut bool,
    buffer: &mut Vec<u8>,
    stack_bytes: u32,
) -> Result<WorkerReply, WorkerError> {
    match command {
        Command::Install(instance) => {
            if *started {
                return Err(WorkerError::AlreadyStarted);
            }
            if kernel.is_some() {
                return Err(WorkerError::AlreadyInstalled);
            }
            if instance.worker_storage().stack_bytes != stack_bytes {
                return Err(WorkerError::StackMismatch);
            }
            if instance.maximum_transfer_bytes() > buffer.capacity() {
                return Err(WorkerError::BufferTooSmall);
            }
            let memory = instance.memory_receipt();
            *kernel = Some(instance);
            return Ok(WorkerReply::Installed(memory));
        }
        Command::Start => {
            if kernel.is_none() {
                return Err(WorkerError::NotInstalled);
            }
            if *started {
                return Err(WorkerError::AlreadyStarted);
            }
            *started = true;
            return Ok(WorkerReply::Started);
        }
        _ => {}
    }
    let kernel = kernel.as_mut().ok_or(WorkerError::NotInstalled)?;
    if !*started {
        return Err(WorkerError::NotStarted);
    }
    match command {
        Command::Step => kernel
            .step()
            .map(WorkerReply::Step)
            .map_err(WorkerError::Execution),
        Command::Offer(cord) => kernel
            .offer_into(cord, buffer)
            .map(|sequence| {
                if sequence.is_none() {
                    buffer.clear();
                }
                WorkerReply::Offer {
                    sequence,
                    bytes: buffer.len(),
                }
            })
            .map_err(WorkerError::Transport),
        Command::Admit { cord, sequence } => kernel
            .admit(cord, sequence, buffer)
            .map(WorkerReply::Ingress)
            .map_err(WorkerError::Transport),
        Command::Delivered { cord, sequence } => kernel
            .delivered(cord, sequence)
            .map(|()| WorkerReply::Delivered)
            .map_err(WorkerError::Transport),
        Command::Terminal(cord) => kernel
            .terminal(cord)
            .map(WorkerReply::Terminal)
            .map_err(WorkerError::Transport),
        Command::Abnormal(cord) => kernel
            .abnormal_terminal(cord)
            .map(WorkerReply::Abnormal)
            .map_err(WorkerError::Transport),
        Command::Close(cord) => kernel
            .close(cord)
            .map(|()| WorkerReply::Closed)
            .map_err(WorkerError::Transport),
        Command::CloseAbnormal(cord, value) => kernel
            .close_abnormal(cord, value)
            .map(|()| WorkerReply::Closed)
            .map_err(WorkerError::Transport),
        Command::Cancel => kernel
            .cancel()
            .map(|()| WorkerReply::Cancelled)
            .map_err(WorkerError::Scheduler),
        Command::Install(_) | Command::Start => unreachable!(),
    }
}
