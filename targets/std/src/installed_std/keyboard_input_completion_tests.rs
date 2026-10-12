//! Finite private ledger mechanics; actual scheduler minting is exercised by Body tests.
use super::*;
use conduit_kernel::{BoundedValueRef, RequestId};

fn event(request: u32, generation: u16) -> CompletedInputEvent {
    CompletedInputEvent {
        descriptor: 0,
        realization: InputRealization::RawKeyboard,
        request: HostCallRequest {
            node: NodeId(1),
            call: HostCallId(0),
            request: RequestId(request),
            input: BoundedValueRef::new(
                ValueRef {
                    slot: 0,
                    generation: 1,
                    byte_len: 0,
                },
                0,
            )
            .unwrap(),
        },
        value: ValueRef {
            slot: 1,
            generation,
            byte_len: 3,
        },
        physical_event: u64::from(request),
    }
}

#[test]
fn completed_event_requires_exact_node_call_slot_generation_and_length() {
    let mut ledger = KeyboardInputCompletions::new();
    ledger.enable().unwrap();
    let original = event(0, 1);
    let value = original.value;
    ledger.record(original).unwrap();
    for (node, call, forged) in [
        (NodeId(2), HostCallId(0), value),
        (NodeId(1), HostCallId(1), value),
        (NodeId(1), HostCallId(0), ValueRef { slot: 2, ..value }),
        (
            NodeId(1),
            HostCallId(0),
            ValueRef {
                generation: 2,
                ..value
            },
        ),
        (
            NodeId(1),
            HostCallId(0),
            ValueRef {
                byte_len: 2,
                ..value
            },
        ),
    ] {
        assert!(ledger.claim(node, call, forged).is_err());
    }
    let proof = ledger.claim(NodeId(1), HostCallId(0), value).unwrap();
    assert_eq!(proof.request.request, RequestId(0));
    assert_eq!(proof.descriptor, 0);
    assert_eq!(proof.realization, InputRealization::RawKeyboard);
}

#[test]
fn later_completion_preserves_older_event_until_exact_single_ack() {
    let mut ledger = KeyboardInputCompletions::new();
    ledger.enable().unwrap();
    let first = event(0, 1).value;
    let second = event(1, 2).value;
    ledger.record(event(0, 1)).unwrap();
    ledger.record(event(1, 2)).unwrap();
    assert!(ledger.acknowledge(NodeId(1), HostCallId(0), first).is_err());
    assert_eq!(
        ledger
            .claim(NodeId(1), HostCallId(0), first)
            .unwrap()
            .physical_event,
        0
    );
    assert!(ledger.claim(NodeId(1), HostCallId(0), first).is_err());
    ledger.acknowledge(NodeId(1), HostCallId(0), first).unwrap();
    assert!(ledger.acknowledge(NodeId(1), HostCallId(0), first).is_err());
    assert!(ledger.claim(NodeId(1), HostCallId(0), first).is_err());
    assert_eq!(
        ledger
            .claim(NodeId(1), HostCallId(0), second)
            .unwrap()
            .physical_event,
        1
    );
}

#[test]
fn capacity_and_revocation_never_silently_replace_completed_event() {
    let mut ledger = KeyboardInputCompletions::new();
    assert!(ledger.record(event(0, 1)).is_err());
    ledger.enable().unwrap();
    let allocations = crate::allocation_probe::begin();
    for request in 0..MAX_COMPLETED_INPUT_EVENTS as u32 {
        ledger.require_capacity(1).unwrap();
        ledger.record(event(request, request as u16 + 1)).unwrap();
    }
    assert_eq!(allocations.finish(), 0);
    assert!(ledger.require_capacity(1).is_err());
    assert!(ledger.record(event(64, 65)).is_err());
    assert!(ledger
        .claim(NodeId(1), HostCallId(0), event(0, 1).value)
        .is_ok());
    ledger.revoke();
    assert!(ledger
        .claim(NodeId(1), HostCallId(0), event(1, 2).value)
        .is_err());
    assert!(ledger
        .acknowledge(NodeId(1), HostCallId(0), event(0, 1).value)
        .is_err());
    assert!(ledger.enable().is_err());
    assert!(ledger.record(event(65, 66)).is_err());
}

#[test]
fn exact_accepted_newer_request_does_not_replace_completed_older_origin() {
    use super::super::keyboard_input_binding::KeyboardRequestAdmission;
    use conduit_body::{Body, BodyPlan, BodyPlayIdentity, BodyPlotPlan, ResidentPlot};
    use conduit_plan_lowering::{
        fragment_set::{lower_local_fragment_set_from_plans, FragmentSetBounds},
        lowering::FIXED_KERNEL_STORAGE_PROFILE,
    };
    let host = crate::BodyLiveForeQueue::parsec_input_test_host(crate::StdHostConfig {
        host_id: "proof/completed-origin".into(),
        boot_id: "proof/completed-origin/boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let plan = crate::BodyLiveForeQueue::authored_parsec_input_plan_on_host(host.advertisement());
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "sign/completed-origin-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "sign/completed-origin-wake".into()).unwrap().1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    let prospective = BodyPlayIdentity::bind(&body_plan, 0);
    let lowered = lower_local_fragment_set_from_plans(
        &[&body_plan.plots[0].plan],
        FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 1,
            nodes: super::super::MAX_NODES as u16,
            cords: super::super::MAX_CORDS as u16,
            queue_slots: super::super::MAX_QUEUE_SLOTS as u16,
            value_bytes: 16 * 1024 * 1024,
            sign_items: 4096,
            sign_bytes: 1024 * 1024,
        },
    )
    .unwrap();
    let identities = lowered
        .partitions
        .iter()
        .map(|part| part.identity.clone())
        .collect::<Vec<_>>();
    let operations = lowered
        .partitions
        .iter()
        .flat_map(|part| part.host_calls.clone())
        .collect::<Vec<_>>();
    let operation = operations
        .iter()
        .find(|operation| {
            InputRealization::from_contract(operation.contract_id.as_str())
                == Some(InputRealization::RawKeyboard)
        })
        .unwrap();
    let mut admission = KeyboardRequestAdmission::prepare_body(
        host.advertisement(),
        &body_plan,
        &prospective,
        &identities,
        &operations,
    )
    .unwrap();
    admission.enable_completed_event_custody().unwrap();
    // Private confirmation mechanism fixture; actual issued Play and scheduler
    // completion minting remain exercised by the installed Body test.
    admission
        .confirm_body_play(host.advertisement(), &prospective)
        .unwrap();
    let mut first = event(0, 1);
    first.request.node = operation.node;
    first.request.call = operation.call;
    admission
        .accept(InputRealization::RawKeyboard, first.request)
        .unwrap();
    first.descriptor = admission
        .completed_origin_index(InputRealization::RawKeyboard, first.request)
        .unwrap();
    let value = first.value;
    admission.completions.record(first).unwrap();
    let mut next = event(1, 2).request;
    next.node = operation.node;
    next.call = operation.call;
    admission
        .accept(InputRealization::RawKeyboard, next)
        .unwrap();
    assert!(admission
        .completed_origin_index(InputRealization::RawKeyboard, next)
        .is_ok());
    assert_eq!(
        admission
            .completions
            .claim(operation.node, operation.call, value)
            .unwrap()
            .request
            .request,
        RequestId(0)
    );
    admission
        .completions
        .acknowledge(operation.node, operation.call, value)
        .unwrap();
    admission.revoke();
    assert!(admission
        .completions
        .claim(operation.node, operation.call, value)
        .is_err());
}
