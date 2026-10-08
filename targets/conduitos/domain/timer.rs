//! The production fixed kernel owns local timer/count progression in private memory.
use crate::{
    frame::TextFrame,
    gate, layout,
    timer_runtime::{PreparedTimerGraph, TourTimerKernel},
};
use conduit_kernel::{
    NodeId, RequestId,
    scheduler::{HostCallRequest, SchedulerStatus},
};
use core::mem::MaybeUninit;

const MAGIC: u64 = 0x5449_4d45_524b_4552;
const STATE: *mut State = layout::RETAINED_ADDRESS as *mut State;
const TIMER_WAIT: u32 = 10;
const COUNT_PRESENT: u32 = 11;

struct State {
    magic: u64,
    kernel: MaybeUninit<TourTimerKernel>,
    timer_handle: u64,
    count_handle: u64,
    timer: Option<HostCallRequest>,
    presentation: Option<HostCallRequest>,
}
const _: () = assert!(core::mem::size_of::<State>() <= layout::RETAINED_BYTES);

pub unsafe fn execute(frame: &mut TextFrame) -> ! {
    if frame.command == 10 {
        let length = frame.input_length as usize;
        if length > frame.input.len() || frame.timer_handle == 0 || frame.count_handle == 0 {
            gate::finish(3);
        }
        let graph =
            PreparedTimerGraph::decode(&frame.input[..length]).unwrap_or_else(|_| gate::finish(3));
        let kernel =
            TourTimerKernel::from_prepared_graph(graph).unwrap_or_else(|_| gate::finish(3));
        // This allocation is private, writable and non-executable on every backend.
        unsafe {
            STATE.write(State {
                magic: MAGIC,
                kernel: MaybeUninit::new(kernel),
                timer_handle: frame.timer_handle,
                count_handle: frame.count_handle,
                timer: None,
                presentation: None,
            });
        }
    }
    let state = unsafe { &mut *STATE };
    if state.magic != MAGIC {
        gate::finish(3);
    }
    let kernel = unsafe { state.kernel.assume_init_mut() };
    match frame.command {
        10 => finish(frame, kernel, 0),
        11 => {
            for _ in 0..192 {
                if let Some(request) = kernel.next_host_request() {
                    let timer = kernel.is_timer(&request);
                    let pending = if timer {
                        &mut state.timer
                    } else {
                        &mut state.presentation
                    };
                    if pending.is_some() || (!timer && !kernel.is_presentation(&request)) {
                        gate::finish(3);
                    }
                    let bytes = kernel
                        .host_value(request.input.value)
                        .unwrap_or_else(|_| gate::finish(3));
                    if bytes.len() != 8 {
                        gate::finish(3);
                    }
                    frame.output.fill(0);
                    if timer {
                        frame.output[..8].copy_from_slice(bytes);
                        frame.output_length = 8;
                    } else {
                        let count = u64::from_le_bytes(
                            bytes.try_into().unwrap_or_else(|_| gate::finish(3)),
                        );
                        frame.output_length = decimal(count, &mut frame.output) as u32;
                    }
                    frame.timer_node = u32::from(request.node.0);
                    frame.timer_request = request.request.0;
                    frame.timer_slot = u32::from(request.input.value.slot);
                    frame.timer_generation = u32::from(request.input.value.generation);
                    frame.timer_value_bytes = request.input.value.byte_len;
                    frame.timer_admitted_bytes = request.input.admitted_bytes;
                    frame.timer_kind = if timer { 1 } else { 2 };
                    frame.capability = if timer {
                        state.timer_handle
                    } else {
                        state.count_handle
                    };
                    frame.operation = if timer { TIMER_WAIT } else { COUNT_PRESENT };
                    frame.work_units = 1;
                    *pending = Some(request);
                    finish(frame, kernel, 0x200);
                }
                match kernel.step().unwrap_or_else(|_| gate::finish(3)) {
                    SchedulerStatus::Progress { .. } => {}
                    SchedulerStatus::Idle => finish(frame, kernel, 1),
                    SchedulerStatus::Cancelled => finish(frame, kernel, 2),
                    SchedulerStatus::Drained => gate::finish(3),
                }
            }
            gate::finish(3);
        }
        12 => {
            let node = u16::try_from(frame.timer_node).unwrap_or_else(|_| gate::finish(3));
            let request = RequestId(frame.timer_request);
            let pending = match frame.timer_kind {
                1 => &mut state.timer,
                2 => &mut state.presentation,
                _ => gate::finish(3),
            };
            if !pending.is_some_and(|expected| {
                expected.node == NodeId(node) && expected.request == request
            }) {
                gate::finish(3);
            }
            kernel
                .complete_request(NodeId(node), request)
                .unwrap_or_else(|_| gate::finish(3));
            *pending = None;
            finish(frame, kernel, 0);
        }
        13 => {
            kernel.cancel().unwrap_or_else(|_| gate::finish(3));
            if kernel.step().unwrap_or_else(|_| gate::finish(3)) != SchedulerStatus::Cancelled {
                gate::finish(3);
            }
            finish(frame, kernel, 2);
        }
        _ => gate::finish(3),
    }
}

fn finish(frame: &mut TextFrame, kernel: &TourTimerKernel, status: u32) -> ! {
    frame.timer_decisions = kernel.decisions();
    frame.timer_signs = u32::from(kernel.sign_count());
    frame.timer_pending = kernel.pending_host_calls() as u32;
    frame.timer_status = status;
    frame.status = if status == 0x200 { 0 } else { status };
    gate::finish(status)
}

fn decimal(mut value: u64, output: &mut [u8; 256]) -> usize {
    let mut digits = [0; 20];
    let mut length = 0;
    loop {
        digits[length] = b'0' + (value % 10) as u8;
        length += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for index in 0..length {
        output[index] = digits[length - index - 1];
    }
    length
}
