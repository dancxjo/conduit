use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LinkBindingId, LinkEndpointId, LinkLimits, PlanId, PROTOCOL_VERSION,
};
use conduit_wire::{
    LineAttachment, SessionBinding, SessionCheckpoint, SessionCheckpointOffer,
    SessionEndpointIdentity, SessionLimits, SessionMachine, SessionMessage, SessionResumeAction,
    SessionRole, SessionTransferCheckpoint, WireError,
};

mod allocation_probe {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    pub struct TrackingAllocator;

    thread_local! {
        static TRACKING: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) };
        }
    }

    fn record() {
        let _ = TRACKING.try_with(|tracking| {
            if tracking.get() {
                let _ = ALLOCATIONS.try_with(|allocations| {
                    allocations.set(allocations.get().saturating_add(1));
                });
            }
        });
    }

    #[global_allocator]
    static ALLOCATOR: TrackingAllocator = TrackingAllocator;

    pub fn allocations_during(action: impl FnOnce()) -> usize {
        ALLOCATIONS.with(|allocations| allocations.set(0));
        TRACKING.with(|tracking| tracking.set(true));
        action();
        TRACKING.with(|tracking| tracking.set(false));
        ALLOCATIONS.with(Cell::get)
    }
}

fn binding() -> SessionBinding {
    let plan_id = PlanId::from("resume/plan");
    let source_host = HostId::from("resume/source-host");
    let source_boot = BootId::from("resume/source-boot");
    let sink_host = HostId::from("resume/sink-host");
    let sink_boot = BootId::from("resume/sink-boot");
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        source_active_play_id: bind_active_play(&plan_id, &source_host, &source_boot, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink_host, &sink_boot, 0).active_play_id,
        plan_id,
        source_fragment_id: FragmentId::from("resume/source-fragment"),
        sink_fragment_id: FragmentId::from("resume/sink-fragment"),
        connection_id: ConnectionId::from("resume/connection"),
        source: SessionEndpointIdentity {
            host_id: source_host.clone(),
            boot_id: source_boot.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink_host.clone(),
            boot_id: sink_boot.clone(),
        },
        value_kind: KindId::from("value/resume@1"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 16,
            maximum_buffered_bytes: 16,
        },
        attachment: LineAttachment {
            line_id: "line/session".into(),
            link_binding_id: LinkBindingId::from("resume/websocket"),
            base: ConnectionBase::WebSocket,
            base_instance_id: ConnectionBaseInstanceId::from("resume/ws-base"),
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from("resume/ws-source"),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from("resume/ws-sink"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 16,
                maximum_buffered_bytes: 16,
                maximum_frame_bytes: 512,
            },
        },
    }
}

fn replacement_binding() -> SessionBinding {
    let mut replacement = binding();
    replacement.attachment.link_binding_id = LinkBindingId::from("resume/usb");
    replacement.attachment.base = ConnectionBase::UsbCdc;
    replacement.attachment.base_instance_id = ConnectionBaseInstanceId::from("resume/usb-base");
    replacement.attachment.source_endpoint_id = LinkEndpointId::from("resume/usb-source");
    replacement.attachment.sink_endpoint_id = LinkEndpointId::from("resume/usb-sink");
    replacement
}

fn trigger(machine: &mut SessionMachine) {
    let exact = machine.binding().clone();
    machine.admit_outbound(exact.hello_frame()).unwrap();
    machine.admit_inbound(exact.hello_frame()).unwrap();
    machine
        .admit_outbound(exact.frame(SessionMessage::Ready))
        .unwrap();
    machine
        .admit_inbound(exact.frame(SessionMessage::Ready))
        .unwrap();
}

#[test]
fn clean_and_finite_in_flight_checkpoints_reconcile_on_a_new_attachment() {
    let original = binding();
    let replacement = replacement_binding();
    let mut clean = SessionMachine::new(original.clone(), SessionRole::Source).unwrap();
    trigger(&mut clean);
    let clean_offer = SessionCheckpointOffer {
        identity: original.identity(),
        checkpoint: clean.checkpoint(),
    };
    let clean_acceptance = clean
        .resume_with_attachment(replacement.clone(), clean_offer)
        .unwrap();
    assert_eq!(clean_acceptance.action, SessionResumeAction::Continue);
    assert!(clean_acceptance.same_plan_continues);
    assert!(!clean.is_active());
    assert_eq!(
        clean.admit_outbound(replacement.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &[1],
        })),
        Err(WireError::InvalidState)
    );

    let mut offered_source = SessionMachine::new(original.clone(), SessionRole::Source).unwrap();
    trigger(&mut offered_source);
    offered_source
        .admit_outbound(original.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &[7],
        }))
        .unwrap();
    assert_eq!(
        offered_source
            .resume_with_attachment(
                replacement.clone(),
                SessionCheckpointOffer {
                    identity: original.identity(),
                    checkpoint: SessionCheckpoint {
                        next_sequence: 0,
                        transfer: SessionTransferCheckpoint::None,
                        input_closed: false,
                    },
                },
            )
            .unwrap()
            .action,
        SessionResumeAction::ReplayOffered(0)
    );

    let mut awaiting_sink = SessionMachine::new(original.clone(), SessionRole::Sink).unwrap();
    trigger(&mut awaiting_sink);
    assert_eq!(
        awaiting_sink
            .resume_with_attachment(
                replacement.clone(),
                SessionCheckpointOffer {
                    identity: original.identity(),
                    checkpoint: SessionCheckpoint {
                        next_sequence: 0,
                        transfer: SessionTransferCheckpoint::Offered(0),
                        input_closed: false,
                    },
                },
            )
            .unwrap()
            .action,
        SessionResumeAction::AwaitReplay(0)
    );

    let mut accepted_source = SessionMachine::new(original.clone(), SessionRole::Source).unwrap();
    trigger(&mut accepted_source);
    accepted_source
        .admit_outbound(original.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &[9],
        }))
        .unwrap();
    accepted_source
        .admit_inbound(original.frame(SessionMessage::Accepted { sequence: 0 }))
        .unwrap();
    assert_eq!(
        accepted_source
            .resume_with_attachment(
                replacement,
                SessionCheckpointOffer {
                    identity: original.identity(),
                    checkpoint: SessionCheckpoint {
                        next_sequence: 1,
                        transfer: SessionTransferCheckpoint::None,
                        input_closed: false,
                    },
                },
            )
            .unwrap()
            .action,
        SessionResumeAction::AdvanceDelivered(0)
    );
    assert_eq!(accepted_source.next_sequence(), 1);
}

#[test]
fn attachment_resume_validates_without_allocating() {
    let original = binding();
    let replacement = replacement_binding();
    let mut machine = SessionMachine::new(original.clone(), SessionRole::Sink).unwrap();
    trigger(&mut machine);
    let peer = SessionCheckpointOffer {
        identity: original.identity(),
        checkpoint: machine.checkpoint(),
    };

    let allocations = allocation_probe::allocations_during(|| {
        machine.resume_with_attachment(replacement, peer).unwrap();
    });

    assert_eq!(allocations, 0);
}

#[test]
fn contradictory_stale_or_different_logical_checkpoints_fail_closed() {
    let original = binding();
    let mut machine = SessionMachine::new(original.clone(), SessionRole::Source).unwrap();
    trigger(&mut machine);
    assert_eq!(
        machine.resume_with_attachment(
            replacement_binding(),
            SessionCheckpointOffer {
                identity: original.identity(),
                checkpoint: SessionCheckpoint {
                    next_sequence: 8,
                    transfer: SessionTransferCheckpoint::Accepted(7),
                    input_closed: false,
                },
            },
        ),
        Err(WireError::InvalidState)
    );

    let mut different_plan = replacement_binding();
    different_plan.plan_id = PlanId::from("resume/different-plan");
    assert_eq!(
        machine.resume_with_attachment(
            different_plan,
            SessionCheckpointOffer {
                identity: original.identity(),
                checkpoint: machine.checkpoint(),
            },
        ),
        Err(WireError::InvalidSession)
    );
}
