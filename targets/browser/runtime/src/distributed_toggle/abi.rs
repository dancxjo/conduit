//! WASM ABI exports for the distributed toggle browser sink.
//!
//! Thread-local state and `#[no_mangle]` extern C functions called from JavaScript
//! to drive the toggle demonstration.

use super::super::FRAME_CAPACITY;
use super::sink::ToggleDistributedSink;
use super::{ERROR_NOT_STARTED, ERROR_PRESENTATION, ERROR_SESSION, OUTPUT_NONE, STATUS_RUNNING};
use std::cell::RefCell;

thread_local! {
    static TOGGLE_SINK: RefCell<Option<ToggleDistributedSink>> = const { RefCell::new(None) };
    static TOGGLE_INPUT: RefCell<[u8; FRAME_CAPACITY]> = const { RefCell::new([0; FRAME_CAPACITY]) };
}

fn with_toggle_sink<T>(
    action: impl FnOnce(&mut ToggleDistributedSink) -> Result<T, i32>,
) -> Result<T, i32> {
    TOGGLE_SINK.with(|slot| {
        let mut slot = slot.borrow_mut();
        action(slot.as_mut().ok_or(ERROR_NOT_STARTED)?)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_start() -> i32 {
    match ToggleDistributedSink::prepare(None) {
        Ok(sink) => {
            TOGGLE_SINK.with(|slot| *slot.borrow_mut() = Some(sink));
            STATUS_RUNNING
        }
        Err(code) => code,
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_status() -> i32 {
    TOGGLE_SINK.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(ToggleDistributedSink::status)
            .unwrap_or(ERROR_NOT_STARTED)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_output_kind() -> i32 {
    TOGGLE_SINK.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| sink.output_kind)
            .unwrap_or(OUTPUT_NONE)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_output_ptr() -> *const u8 {
    TOGGLE_SINK.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| sink.output.as_ptr())
            .unwrap_or(core::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_output_len() -> u32 {
    TOGGLE_SINK.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| sink.output_len as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_input_ptr() -> *mut u8 {
    TOGGLE_INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_input_capacity() -> u32 {
    FRAME_CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_ingest(length: u32) -> i32 {
    let length = length as usize;
    if length > FRAME_CAPACITY {
        return ERROR_SESSION;
    }
    TOGGLE_INPUT.with(|input| {
        let input = input.borrow();
        with_toggle_sink(|sink| sink.ingest(&input[..length]))
            .map(|_| conduit_browser_toggle_distributed_status())
            .unwrap_or_else(|code| code)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_advance() -> i32 {
    with_toggle_sink(ToggleDistributedSink::advance)
        .map(|_| conduit_browser_toggle_distributed_status())
        .unwrap_or_else(|code| code)
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_clear_output() -> i32 {
    with_toggle_sink(|sink| {
        sink.clear_output();
        Ok(())
    })
    .map(|_| conduit_browser_toggle_distributed_status())
    .unwrap_or_else(|code| code)
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_complete(length: u32) -> i32 {
    let length = length as usize;
    if length > FRAME_CAPACITY {
        return ERROR_PRESENTATION;
    }
    TOGGLE_INPUT.with(|input| {
        let input = input.borrow();
        with_toggle_sink(|sink| sink.complete_presentation(&input[..length]))
            .map(|_| conduit_browser_toggle_distributed_status())
            .unwrap_or_else(|code| code)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_cancel() -> i32 {
    with_toggle_sink(ToggleDistributedSink::cancel)
        .map(|_| conduit_browser_toggle_distributed_status())
        .unwrap_or_else(|code| code)
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_receipt_count() -> u32 {
    TOGGLE_SINK.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| sink.receipts as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_toggle_distributed_capacity_stable() -> u32 {
    TOGGLE_SINK.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| u32::from(sink.capacity_seal() == sink.seal))
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::super::sink::ToggleDistributedSink;
    use super::super::OUTPUT_PRESENT;
    use conduit_core::{InfoBool, BOOL_ENCODED_LEN};
    use conduit_signal_conformance::DISTRIBUTED_MAXIMUM_FRAME_BYTES;
    use conduit_wire::{encode_session_frame_into, SessionFrame, SessionMessage};

    fn ingest(sink: &mut ToggleDistributedSink, frame: SessionFrame<'_>) {
        let mut bytes = [0_u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize];
        let length = encode_session_frame_into(
            frame,
            &mut bytes,
            BOOL_ENCODED_LEN as u32,
            DISTRIBUTED_MAXIMUM_FRAME_BYTES,
        )
        .expect("session frame encodes");
        sink.ingest(&bytes[..length])
            .expect("session frame ingests");
    }

    #[test]
    fn toggle_browser_reconstructs_exact_sink_fragment_and_session() {
        let sink = ToggleDistributedSink::prepare(None).expect("toggle sink prepares");
        assert_eq!(sink.fragment.placements.len(), 1);
        assert_eq!(sink.lowered.remote_endpoints.len(), 1);
        assert_eq!(sink.binding.plan_id, sink.fragment.plan_id);
        assert_eq!(sink.binding.sink_fragment_id, sink.fragment.fragment_id);
        assert_eq!(sink.capacity_seal(), sink.seal);
    }

    #[test]
    fn first_accepted_value_drives_presentation_before_delivery() {
        let mut sink = ToggleDistributedSink::prepare(None).expect("toggle sink prepares");
        let binding = sink.binding.clone();
        ingest(&mut sink, binding.hello_frame());
        ingest(&mut sink, binding.frame(SessionMessage::Ready));

        let value = InfoBool::TRUE.encode();
        ingest(
            &mut sink,
            binding.frame(SessionMessage::Offered {
                sequence: 0,
                payload: &value,
            }),
        );
        sink.advance().expect("accepted value advances");

        assert_eq!(sink.output_kind, OUTPUT_PRESENT);
    }
}
