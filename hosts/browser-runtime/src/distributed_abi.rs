use super::*;

const MAXIMUM_SOURCE_ID_BYTES: usize = 128;

thread_local! {
    static DISTRIBUTED_SOURCE_IDENTITY: RefCell<Option<(HostId, BootId)>> = const { RefCell::new(None) };
}

fn with_sink<T>(action: impl FnOnce(&mut DistributedSink) -> Result<T, i32>) -> Result<T, i32> {
    DISTRIBUTED.with(|slot| {
        let mut slot = slot.borrow_mut();
        action(slot.as_mut().ok_or(ERROR_NOT_STARTED)?)
    })
}

fn start(kind: PlanKind) -> i32 {
    let source_identity = DISTRIBUTED_SOURCE_IDENTITY.with(|identity| identity.borrow_mut().take());
    match DistributedSink::prepare(None, kind, source_identity) {
        Ok(sink) => {
            DISTRIBUTED.with(|slot| *slot.borrow_mut() = Some(sink));
            STATUS_RUNNING
        }
        Err(code) => code,
    }
}

/// Configures the exact native source host/boot identity before starting the
/// browser peer. Bytes are read as adjacent UTF-8 host then boot identities
/// from the existing bounded input buffer.
#[no_mangle]
pub extern "C" fn conduit_browser_distributed_configure_source(
    host_length: u32,
    boot_length: u32,
) -> i32 {
    DISTRIBUTED_SOURCE_IDENTITY.with(|identity| {
        identity.borrow_mut().take();
    });
    let host_length = host_length as usize;
    let boot_length = boot_length as usize;
    let Some(total_length) = host_length.checked_add(boot_length) else {
        return ERROR_PREPARE;
    };
    if host_length == 0
        || boot_length == 0
        || host_length > MAXIMUM_SOURCE_ID_BYTES
        || boot_length > MAXIMUM_SOURCE_ID_BYTES
        || total_length > FRAME_CAPACITY
    {
        return ERROR_PREPARE;
    }
    DISTRIBUTED_INPUT.with(|input| {
        let input = input.borrow();
        let host = core::str::from_utf8(&input[..host_length]);
        let boot = core::str::from_utf8(&input[host_length..total_length]);
        match (host, boot) {
            (Ok(host), Ok(boot)) => {
                DISTRIBUTED_SOURCE_IDENTITY.with(|identity| {
                    *identity.borrow_mut() = Some((HostId::from(host), BootId::from(boot)));
                });
                STATUS_RUNNING
            }
            _ => ERROR_PREPARE,
        }
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_identity(host: &str, boot: &str) {
        DISTRIBUTED_INPUT.with(|input| {
            let mut input = input.borrow_mut();
            input[..host.len()].copy_from_slice(host.as_bytes());
            input[host.len()..host.len() + boot.len()].copy_from_slice(boot.as_bytes());
        });
    }

    #[test]
    fn configured_identity_is_exact_and_invalid_reconfiguration_clears_it() {
        let host = "patchbay/native-test";
        let boot = "patchbay/boot-test";
        write_identity(host, boot);
        assert_eq!(
            conduit_browser_distributed_configure_source(host.len() as u32, boot.len() as u32),
            STATUS_RUNNING
        );
        assert_eq!(conduit_browser_distributed_start(), STATUS_RUNNING);
        DISTRIBUTED.with(|slot| {
            let slot = slot.borrow();
            let binding = &slot.as_ref().expect("sink").binding;
            assert_eq!(binding.source.host_id.as_str(), host);
            assert_eq!(binding.source.boot_id.as_str(), boot);
        });

        write_identity(host, boot);
        assert_eq!(
            conduit_browser_distributed_configure_source(129, boot.len() as u32),
            ERROR_PREPARE
        );
        assert_eq!(conduit_browser_distributed_start(), STATUS_RUNNING);
        DISTRIBUTED.with(|slot| {
            let slot = slot.borrow();
            assert_eq!(
                slot.as_ref().expect("sink").binding.source.host_id.as_str(),
                conduit_signal::DISTRIBUTED_STD_HOST_ID
            );
        });
    }
}
