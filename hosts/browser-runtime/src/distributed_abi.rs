use super::*;

fn with_sink<T>(action: impl FnOnce(&mut DistributedSink) -> Result<T, i32>) -> Result<T, i32> {
    DISTRIBUTED.with(|slot| {
        let mut slot = slot.borrow_mut();
        action(slot.as_mut().ok_or(ERROR_NOT_STARTED)?)
    })
}

fn start(kind: PlanKind) -> i32 {
    match DistributedSink::prepare(None, kind) {
        Ok(sink) => {
            DISTRIBUTED.with(|slot| *slot.borrow_mut() = Some(sink));
            STATUS_RUNNING
        }
        Err(code) => code,
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_start() -> i32 {
    start(PlanKind::StdBrowser)
}

#[no_mangle]
pub extern "C" fn conduit_browser_triple_start() -> i32 {
    start(PlanKind::Triple)
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_status() -> i32 {
    DISTRIBUTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(DistributedSink::status)
            .unwrap_or(ERROR_NOT_STARTED)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_output_kind() -> i32 {
    DISTRIBUTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| sink.output_kind)
            .unwrap_or(OUTPUT_NONE)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_output_ptr() -> *const u8 {
    DISTRIBUTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| sink.output.as_ptr())
            .unwrap_or(core::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_output_len() -> u32 {
    DISTRIBUTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| sink.output_len as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_input_ptr() -> *mut u8 {
    DISTRIBUTED_INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_input_capacity() -> u32 {
    FRAME_CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_ingest(length: u32) -> i32 {
    let length = length as usize;
    if length > FRAME_CAPACITY {
        return ERROR_SESSION;
    }
    DISTRIBUTED_INPUT.with(|input| {
        let input = input.borrow();
        with_sink(|sink| sink.ingest(&input[..length]))
            .map(|_| conduit_browser_distributed_status())
            .unwrap_or_else(|code| code)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_advance() -> i32 {
    with_sink(DistributedSink::advance)
        .map(|_| conduit_browser_distributed_status())
        .unwrap_or_else(|code| code)
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_clear_output() -> i32 {
    with_sink(|sink| {
        sink.clear_output();
        Ok(())
    })
    .map(|_| conduit_browser_distributed_status())
    .unwrap_or_else(|code| code)
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_complete(length: u32) -> i32 {
    let length = length as usize;
    if length > FRAME_CAPACITY {
        return ERROR_PRESENTATION;
    }
    DISTRIBUTED_INPUT.with(|input| {
        let input = input.borrow();
        with_sink(|sink| sink.complete_presentation(&input[..length]))
            .map(|_| conduit_browser_distributed_status())
            .unwrap_or_else(|code| code)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_cancel() -> i32 {
    with_sink(DistributedSink::cancel)
        .map(|_| conduit_browser_distributed_status())
        .unwrap_or_else(|code| code)
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_receipt_count() -> u32 {
    DISTRIBUTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| sink.receipts as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_pressure_retries() -> u32 {
    DISTRIBUTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| sink.pressure_retries)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_capacity_stable() -> u32 {
    DISTRIBUTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| u32::from(sink.capacity_seal() == sink.seal))
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_retained_values() -> u32 {
    DISTRIBUTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|sink| u32::from(sink.scheduler.values().used_items()))
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_distributed_in_flight_items() -> u32 {
    DISTRIBUTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|sink| {
                let (_, cord) = sink.remote();
                sink.scheduler
                    .cord_usage(cord)
                    .ok()
                    .map(|(items, _)| u32::from(items))
            })
            .unwrap_or(0)
    })
}
