use conduit_core::{
    BlockingFairness, BoundedFlowQueue, CompatibilityOutcome, FlowCapacity, FlowEventKind,
    FlowOffer, FlowPolicy, FlowPolicyReason, FlowQueueState, FlowTypeFacts, FlowWatermarks, Id,
    OfferDisposition, Pressure, SampleSchedule, TraitProof,
};

const RELATIONS: [Id<'static>; 1] = [Id("fixture/replace-latest")];

fn capacity() -> FlowCapacity {
    FlowCapacity::new(2, 4, 8).unwrap()
}

fn policy(name: &str, parameter: &str) -> FlowPolicy<'static> {
    let capacity = capacity();
    let pressure = match name {
        "block" => Pressure::Block(BlockingFairness::Fifo),
        "reject" => Pressure::Reject,
        "coalesce" => Pressure::Coalesce {
            relation: Id("fixture/replace-latest"),
        },
        "sample" => {
            let (every, offset) = parameter.split_once(':').unwrap();
            Pressure::Sample(
                SampleSchedule::new(every.parse().unwrap(), offset.parse().unwrap()).unwrap(),
            )
        }
        "drop-disposable" => Pressure::DropDisposable,
        "disconnect" => Pressure::Disconnect,
        "fail" => Pressure::Fail,
        value => panic!("unknown pressure fixture: {value}"),
    };
    FlowPolicy::new(
        capacity,
        pressure,
        FlowWatermarks::new(1, 2, capacity).unwrap(),
    )
    .unwrap()
}

fn facts(value: &str) -> FlowTypeFacts<'static> {
    match value {
        "any" => FlowTypeFacts {
            disposable: TraitProof::Indeterminate,
            coalescers: None,
        },
        "coalesce-proven" => FlowTypeFacts {
            disposable: TraitProof::Indeterminate,
            coalescers: Some(&RELATIONS),
        },
        "coalesce-unknown" => FlowTypeFacts {
            disposable: TraitProof::Indeterminate,
            coalescers: None,
        },
        "coalesce-missing" => FlowTypeFacts {
            disposable: TraitProof::Indeterminate,
            coalescers: Some(&[]),
        },
        "disposable-proven" => FlowTypeFacts {
            disposable: TraitProof::Proven,
            coalescers: None,
        },
        "disposable-unknown" => FlowTypeFacts {
            disposable: TraitProof::Indeterminate,
            coalescers: None,
        },
        "disposable-forbidden" => FlowTypeFacts {
            disposable: TraitProof::Disproven,
            coalescers: None,
        },
        value => panic!("unknown type facts: {value}"),
    }
}

fn resolved_facts(policy: FlowPolicy<'_>) -> FlowTypeFacts<'static> {
    match policy.pressure {
        Pressure::Coalesce { .. } => facts("coalesce-proven"),
        Pressure::DropDisposable => facts("disposable-proven"),
        _ => facts("any"),
    }
}

fn outcome(value: &str) -> CompatibilityOutcome {
    match value {
        "compatible" => CompatibilityOutcome::Compatible,
        "incompatible" => CompatibilityOutcome::Incompatible,
        "indeterminate" => CompatibilityOutcome::Indeterminate,
        value => panic!("unknown outcome: {value}"),
    }
}

fn event_name(kind: FlowEventKind) -> &'static str {
    match kind {
        FlowEventKind::PressureEntered => "pressure-entered",
        FlowEventKind::PressureCleared => "pressure-cleared",
        FlowEventKind::ValueRejected => "value-rejected",
        FlowEventKind::ValueCoalesced { .. } => "value-coalesced",
        FlowEventKind::ValueSampledOut => "value-sampled-out",
        FlowEventKind::ValueDroppedDisposable => "value-dropped-disposable",
        FlowEventKind::ConsumerReady => "consumer-ready",
        FlowEventKind::ProducerReady => "producer-ready",
        FlowEventKind::Disconnected => "disconnected",
        FlowEventKind::Failed => "failed",
        FlowEventKind::Cancelled { .. } => "cancelled",
    }
}

fn state_name(state: FlowQueueState) -> &'static str {
    match state {
        FlowQueueState::Active => "active",
        FlowQueueState::Disconnected => "disconnected",
        FlowQueueState::Failed => "failed",
        FlowQueueState::Cancelled => "cancelled",
    }
}

#[test]
fn every_policy_matches_the_full_buffer_fixture() {
    let fixtures = include_str!("../../../conformance/c2/flow-policy-v1.tsv");
    for line in fixtures.lines().filter(|line| !line.starts_with('#')) {
        let columns = line.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 11, "invalid fixture row: {line}");
        let policy = policy(columns[1], columns[2]);
        let decision = policy.assess_type_facts(facts(columns[3]));
        assert_eq!(
            decision.outcome,
            outcome(columns[4]),
            "{} outcome",
            columns[0]
        );
        assert_eq!(
            decision.reason.as_str(),
            columns[5],
            "{} reason",
            columns[0]
        );
        if columns[6] == "none" {
            continue;
        }

        let mut slots: [Option<(&str, u32)>; 2] = [None, None];
        let mut queue = BoundedFlowQueue::new(&mut slots, policy, resolved_facts(policy)).unwrap();
        let mut event_names = Vec::new();
        for value in ["a", "b"] {
            let transition = queue.offer(
                value,
                FlowOffer {
                    size_bytes: 4,
                    coalesce_target: None,
                },
            );
            event_names.extend(transition.events.iter().map(|event| event_name(event.kind)));
            assert_eq!(transition.disposition, OfferDisposition::Enqueued);
        }
        let transition = queue.offer(
            "c",
            FlowOffer {
                size_bytes: 4,
                coalesce_target: Some(1),
            },
        );
        event_names.extend(transition.events.iter().map(|event| event_name(event.kind)));
        let disposition = match transition.disposition {
            OfferDisposition::Pending(_) => "pending",
            OfferDisposition::Rejected(_) => "rejected",
            OfferDisposition::Coalesced { .. } => "coalesced",
            OfferDisposition::Dropped(_) => "dropped",
            OfferDisposition::Disconnected(_) => "disconnected",
            OfferDisposition::Failed(_) => "failed",
            value => panic!("unexpected full disposition: {value:?}"),
        };
        assert_eq!(disposition, columns[7], "{} disposition", columns[0]);
        assert_eq!(event_names.join(","), columns[8], "{} events", columns[0]);
        assert!(queue.occupancy_items() <= policy.capacity.items());
        assert!(queue.occupancy_bytes() <= policy.capacity.max_queued_bytes());
        assert_eq!(
            state_name(queue.state()),
            columns[10],
            "{} state",
            columns[0]
        );

        let mut values = Vec::new();
        while let Some(value) = queue.pop().value {
            values.push(value);
        }
        assert_eq!(values.join(","), columns[9], "{} queue order", columns[0]);
    }
}

#[test]
fn capacity_property_holds_for_all_short_operation_traces() {
    let policies = [
        policy("block", "-"),
        policy("reject", "-"),
        policy("coalesce", "fixture/replace-latest"),
        policy("sample", "2:0"),
        policy("drop-disposable", "-"),
        policy("disconnect", "-"),
        policy("fail", "-"),
    ];
    for policy in policies {
        for trace in 0_u32..256 {
            let mut slots = [None, None];
            let mut queue =
                BoundedFlowQueue::new(&mut slots, policy, resolved_facts(policy)).unwrap();
            for step in 0..8 {
                if trace & (1 << step) == 0 {
                    let _ = queue.offer(
                        step,
                        FlowOffer {
                            size_bytes: (step % 4) + 1,
                            coalesce_target: Some(0),
                        },
                    );
                } else {
                    let _ = queue.pop();
                }
                assert!(queue.occupancy_items() <= policy.capacity.items());
                assert!(queue.occupancy_bytes() <= policy.capacity.max_queued_bytes());
            }
        }
    }
}

#[test]
fn every_loss_is_evidenced_and_sampling_is_explicit() {
    let sample = policy("sample", "2:0");
    let mut slots = [None, None];
    let mut queue = BoundedFlowQueue::new(&mut slots, sample, resolved_facts(sample)).unwrap();
    assert_eq!(
        queue
            .offer(
                "selected",
                FlowOffer {
                    size_bytes: 1,
                    coalesce_target: None,
                }
            )
            .disposition,
        OfferDisposition::Enqueued
    );
    let ignored = queue.offer(
        "ignored",
        FlowOffer {
            size_bytes: 1,
            coalesce_target: None,
        },
    );
    assert!(matches!(ignored.disposition, OfferDisposition::Dropped(_)));
    assert!(
        ignored
            .events
            .iter()
            .any(|event| event.kind == FlowEventKind::ValueSampledOut)
    );
}

#[test]
fn cancellation_wakes_blocked_producer_and_waiting_consumer() {
    let block = policy("block", "-");
    let mut producer_slots = [None, None];
    let mut producer_queue =
        BoundedFlowQueue::new(&mut producer_slots, block, resolved_facts(block)).unwrap();
    for value in [1, 2] {
        let _ = producer_queue.offer(
            value,
            FlowOffer {
                size_bytes: 1,
                coalesce_target: None,
            },
        );
    }
    let pending = producer_queue.offer(
        3,
        FlowOffer {
            size_bytes: 1,
            coalesce_target: None,
        },
    );
    assert!(matches!(pending.disposition, OfferDisposition::Pending(3)));
    assert!(producer_queue.cancel().iter().any(|event| matches!(
        event.kind,
        FlowEventKind::Cancelled {
            wake_producer: true,
            ..
        }
    )));

    let mut consumer_slots: [Option<(u8, u32)>; 2] = [None, None];
    let mut consumer_queue =
        BoundedFlowQueue::new(&mut consumer_slots, block, resolved_facts(block)).unwrap();
    assert_eq!(consumer_queue.pop().value, None);
    assert!(consumer_queue.cancel().iter().any(|event| matches!(
        event.kind,
        FlowEventKind::Cancelled {
            wake_consumer: true,
            ..
        }
    )));
}

#[test]
fn pressure_clearance_and_retry_wakes_are_ordered() {
    let block = policy("block", "-");
    let mut slots = [None, None];
    let mut queue = BoundedFlowQueue::new(&mut slots, block, resolved_facts(block)).unwrap();
    for value in [1, 2] {
        let _ = queue.offer(
            value,
            FlowOffer {
                size_bytes: 1,
                coalesce_target: None,
            },
        );
    }
    let _ = queue.offer(
        3,
        FlowOffer {
            size_bytes: 1,
            coalesce_target: None,
        },
    );
    let pop = queue.pop();
    assert_eq!(
        pop.events
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        vec![FlowEventKind::PressureCleared, FlowEventKind::ProducerReady]
    );

    let mut empty_slots = [None, None];
    let mut empty_queue =
        BoundedFlowQueue::new(&mut empty_slots, block, resolved_facts(block)).unwrap();
    let _ = empty_queue.pop();
    let offer = empty_queue.offer(
        1,
        FlowOffer {
            size_bytes: 1,
            coalesce_target: None,
        },
    );
    assert!(
        offer
            .events
            .iter()
            .any(|event| event.kind == FlowEventKind::ConsumerReady)
    );
}

#[test]
fn invalid_capacity_watermarks_and_schedules_are_rejected() {
    assert!(FlowCapacity::new(0, 1, 1).is_err());
    assert!(FlowCapacity::new(1, 8, 4).is_err());
    let capacity = capacity();
    assert!(FlowWatermarks::new(2, 2, capacity).is_err());
    assert!(SampleSchedule::new(0, 0).is_err());
    assert!(SampleSchedule::new(2, 2).is_err());
    assert_eq!(
        policy("block", "-").assess_type_facts(facts("any")).reason,
        FlowPolicyReason::Accepted
    );

    let drop_policy = policy("drop-disposable", "-");
    let mut slots: [Option<(u8, u32)>; 2] = [None, None];
    assert!(BoundedFlowQueue::new(&mut slots, drop_policy, facts("disposable-unknown")).is_err());
}
