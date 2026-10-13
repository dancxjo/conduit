//! Checked descriptor/current scope mechanics; not scheduler completion or map grants.
use super::super::keyboard_input_completion::CompletedInputEvent;
use super::*;
use conduit_body::{BodyPlan, BodyPlayIdentity};
use conduit_kernel::{RequestId, ValueRef};

fn fixture() -> (
    KeyboardRequestAdmission,
    HostAdvertisement,
    BodyPlan,
    BodyPlayIdentity,
    CompletedInputEvent,
) {
    let (host, plan, _) = super::body_tests::body_plan();
    let play = BodyPlayIdentity::bind(&plan, 0);
    let (identities, operations) = super::body_tests::origins(&plan);
    let operation = operations
        .iter()
        .find(|operation| {
            InputRealization::from_contract(operation.contract_id.as_str())
                == Some(InputRealization::SpaceButton)
        })
        .unwrap();
    let mut admission =
        KeyboardRequestAdmission::prepare_body(&host, &plan, &play, &identities, &operations)
            .unwrap();
    admission.enable_completed_event_custody().unwrap();
    admission.confirm_body_play(&host, &play).unwrap();
    let request = super::body_tests::request(operation);
    admission
        .accept(InputRealization::SpaceButton, request)
        .unwrap();
    let event = CompletedInputEvent {
        descriptor: admission
            .completed_origin_index(InputRealization::SpaceButton, request)
            .unwrap(),
        realization: InputRealization::SpaceButton,
        request,
        value: ValueRef {
            slot: 1,
            generation: 1,
            byte_len: 3,
        },
        physical_event: 0,
        input_view_epoch: Some(17),
    };
    (admission, host, plan, play, event)
}

#[test]
fn older_completed_origin_remains_valid_after_next_exact_request() {
    let (mut admission, host, plan, play, event) = fixture();
    let mut next = event.request;
    next.request = RequestId(1);
    admission
        .accept(InputRealization::SpaceButton, next)
        .unwrap();
    let allocations = crate::allocation_probe::begin();
    assert_eq!(
        admission
            .require_completed_event_scope(&event, &plan, &play, &host)
            .unwrap(),
        &plan.plots[0].plan.plan_id
    );
    assert_eq!(allocations.finish(), 0);
    assert_eq!(event.input_view_epoch, Some(17));
    admission.revoke();
    assert!(admission
        .require_completed_event_scope(&event, &plan, &play, &host)
        .is_err());
}

#[test]
fn stale_host_offer_boot_generation_play_and_plan_scope_refuse() {
    let (admission, host, plan, play, event) = fixture();
    for changed in [
        {
            let mut changed = host.clone();
            changed.boot_id = "stale/boot".into();
            changed
        },
        {
            let mut changed = host.clone();
            changed.offer_generation.0 += 1;
            changed
        },
        {
            let mut changed = host.clone();
            changed.capabilities.pop();
            changed
        },
    ] {
        assert!(admission
            .require_completed_event_scope(&event, &plan, &play, &changed)
            .is_err());
    }
    assert!(admission
        .require_completed_event_scope(&event, &plan, &BodyPlayIdentity::bind(&plan, 1), &host)
        .is_err());
    let mut changed = plan.clone();
    changed.workload_revision += 1;
    assert!(admission
        .require_completed_event_scope(&event, &changed, &play, &host)
        .is_err());
}

#[test]
fn forged_descriptor_realization_node_call_and_future_request_refuse() {
    for variant in 0..5 {
        let (admission, host, plan, play, mut event) = fixture();
        match variant {
            0 => event.descriptor = usize::MAX,
            1 => event.realization = InputRealization::RawKeyboard,
            2 => event.request.node.0 += 1,
            3 => event.request.call.0 += 1,
            _ => event.request.request = RequestId(1),
        }
        assert!(admission
            .require_completed_event_scope(&event, &plan, &play, &host)
            .is_err());
    }
}
