use super::*;

/// Raw transfer/event injection under the serialized synthetic controller fixture.
#[cfg(test)]
pub(crate) unsafe fn synthetic_transfer_event(
    controller: &XhciReady,
    event: Event,
    stop_code: Option<u8>,
) {
    let encode = |event: Event, cycle: u32| {
        [
            event.pointer as u32,
            (event.pointer >> 32) as u32,
            (u32::from(event.completion_code) << 24) | event.residual,
            (u32::from(event.event_type) << 10)
                | (u32::from(event.slot) << 24)
                | (u32::from(event.endpoint) << 16)
                | cycle,
        ]
    };
    // SAFETY: caller owns with_synthetic_stop_controller's mutex and actual
    // original owner; this only stages raw controller observations below physics.
    unsafe {
        DMA.event_ring[controller.event_dequeue] = encode(event, controller.event_cycle);
        if let Some(code) = stop_code {
            let next = (controller.event_dequeue + 1) % EVENT_TRBS;
            let cycle = controller.event_cycle ^ u32::from(next == 0);
            let pointer = controller.dma_physical
                + core::mem::offset_of!(DmaStorage, command_ring) as u64
                + controller.command_enqueue as u64 * 16;
            DMA.event_ring[next] = encode(
                Event {
                    event_type: 33,
                    completion_code: code,
                    slot: 1,
                    endpoint: 0,
                    residual: 0,
                    pointer,
                },
                cycle,
            );
        }
    }
}

/// Stage acknowledgement for the next command issued by this original owner.
#[cfg(test)]
pub(crate) unsafe fn synthetic_next_stop_ack(controller: &XhciReady) {
    let pointer = controller.dma_physical
        + core::mem::offset_of!(DmaStorage, command_ring) as u64
        + controller.command_enqueue as u64 * 16;
    // SAFETY: caller holds the synthetic controller mutex and original owner.
    unsafe {
        synthetic_transfer_event(
            controller,
            Event {
                event_type: 33,
                completion_code: 1,
                slot: 1,
                endpoint: 0,
                residual: 0,
                pointer,
            },
            None,
        );
    }
}
