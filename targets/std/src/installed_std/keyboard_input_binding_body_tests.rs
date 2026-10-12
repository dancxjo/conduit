//! Body input custody through prospective preparation and actual issued execution.
use super::*;
use conduit_body::{Body, BodyPlan, BodyPlayIdentity, BodyPlotPlan, ResidentPlot};
use conduit_core::SignId;
use conduit_kernel::{BoundedValueRef, RequestId, ValueRef};

pub(super) fn body_plan() -> (HostAdvertisement, BodyPlan, conduit_body::Wake) {
    let (advertisement, plan) = super::tests::planned_plan();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("sign/input-origin-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/input-origin-wake"))
        .unwrap()
        .1;
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
    (advertisement, body_plan, wake)
}

pub(super) fn origins(plan: &BodyPlan) -> (Vec<KernelIdentityMap>, Vec<LoweredHostCall>) {
    use conduit_plan_lowering::{
        fragment_set::{lower_local_fragment_set_from_plans, FragmentSetBounds},
        lowering::FIXED_KERNEL_STORAGE_PROFILE,
    };
    let lowered = lower_local_fragment_set_from_plans(
        &plan
            .plots
            .iter()
            .map(|partition| &partition.plan)
            .collect::<Vec<_>>(),
        FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 8,
            nodes: super::super::MAX_NODES as u16,
            cords: super::super::MAX_CORDS as u16,
            queue_slots: super::super::MAX_QUEUE_SLOTS as u16,
            value_bytes: 16 * 1024 * 1024,
            sign_items: 4096,
            sign_bytes: 1024 * 1024,
        },
    )
    .unwrap();
    let operations = lowered
        .partitions
        .iter()
        .flat_map(|partition| partition.host_calls.clone())
        .collect();
    let identities = lowered
        .partitions
        .into_iter()
        .map(|partition| partition.identity)
        .collect();
    (identities, operations)
}

pub(super) fn request(operation: &LoweredHostCall) -> HostCallRequest {
    HostCallRequest {
        node: operation.node,
        call: operation.call,
        request: RequestId(0),
        input: BoundedValueRef::new(
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 0,
            },
            0,
        )
        .unwrap(),
    }
}

#[test]
fn prospective_only_and_wrong_issued_body_play_cannot_admit_input() {
    let (advertisement, plan, _) = body_plan();
    let prospective = BodyPlayIdentity::bind(&plan, 0);
    let (identities, operations) = origins(&plan);
    let operation = operations
        .iter()
        .find(|operation| {
            InputRealization::from_contract(operation.contract_id.as_str())
                == Some(InputRealization::SpaceButton)
        })
        .unwrap();
    let mut unissued = KeyboardRequestAdmission::prepare_body(
        &advertisement,
        &plan,
        &prospective,
        &identities,
        &operations,
    )
    .unwrap();
    assert!(unissued
        .accept(InputRealization::SpaceButton, request(operation))
        .is_err());
    assert!(unissued
        .confirm_body_play(&advertisement, &prospective)
        .is_err());
    let mut admission = KeyboardRequestAdmission::prepare_body(
        &advertisement,
        &plan,
        &prospective,
        &identities,
        &operations,
    )
    .unwrap();
    let wrong = BodyPlayIdentity::bind(&plan, 1);
    assert!(admission.confirm_body_play(&advertisement, &wrong).is_err());
    assert!(admission
        .accept(InputRealization::SpaceButton, request(operation))
        .is_err());
}

#[test]
fn body_boot_generation_partition_and_revoked_confirmation_refuse() {
    let (advertisement, plan, _) = body_plan();
    let prospective = BodyPlayIdentity::bind(&plan, 0);
    let (identities, operations) = origins(&plan);
    for boot in [true, false] {
        let mut changed = advertisement.clone();
        if boot {
            changed.boot_id = "changed/body-boot".into();
        } else {
            changed.offer_generation.0 += 1;
        }
        assert!(KeyboardRequestAdmission::prepare_body(
            &changed,
            &plan,
            &prospective,
            &identities,
            &operations
        )
        .is_err());
        let mut admission = KeyboardRequestAdmission::prepare_body(
            &advertisement,
            &plan,
            &prospective,
            &identities,
            &operations,
        )
        .unwrap();
        assert!(admission.confirm_body_play(&changed, &prospective).is_err());
    }
    let mut changed = identities.clone();
    changed[0].plan_id = "different/input-partition".into();
    assert!(KeyboardRequestAdmission::prepare_body(
        &advertisement,
        &plan,
        &prospective,
        &changed,
        &operations
    )
    .is_err());
    let mut admission = KeyboardRequestAdmission::prepare_body(
        &advertisement,
        &plan,
        &prospective,
        &identities,
        &operations,
    )
    .unwrap();
    admission
        .confirm_body_play(&advertisement, &prospective)
        .unwrap();
    let operation = operations
        .iter()
        .find(|operation| {
            InputRealization::from_contract(operation.contract_id.as_str())
                == Some(InputRealization::SpaceButton)
        })
        .unwrap();
    admission
        .accept(InputRealization::SpaceButton, request(operation))
        .unwrap();
    assert!(admission.is_current(InputRealization::SpaceButton));
    admission.revoke();
    assert!(!admission.is_current(InputRealization::SpaceButton));
    assert!(admission
        .confirm_body_play(&advertisement, &prospective)
        .is_err());
}

#[test]
fn each_body_partition_retains_its_exact_input_plan_and_global_node() {
    // Descriptor custody only. KeyboardInputHost still has one pending request
    // per realization; this does not claim concurrent same-realization sources.
    let (advertisement, first) = super::tests::planned_plan_named("first_input");
    let (_, second) = super::tests::planned_plan_on_host("second_input", &advertisement);
    let body = Body::born(
        first.source_document_id.clone(),
        first.checked_plot_id.clone(),
        1,
        SignId::from("sign/two-inputs-born"),
    )
    .unwrap()
    .admit_plot(
        ResidentPlot::new(
            second.source_document_id.clone(),
            second.checked_plot_id.clone(),
        ),
        SignId::from("sign/second-input-admit"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/two-inputs-wake"))
        .unwrap()
        .1;
    let plan = BodyPlan::seal(
        &wake,
        [first, second]
            .into_iter()
            .map(|plan| BodyPlotPlan {
                plot: ResidentPlot::new(
                    plan.source_document_id.clone(),
                    plan.checked_plot_id.clone(),
                ),
                plan,
            })
            .collect(),
    )
    .unwrap();
    let prospective = BodyPlayIdentity::bind(&plan, 0);
    let (identities, operations) = origins(&plan);
    let inputs: Vec<_> = operations
        .iter()
        .filter(|operation| {
            InputRealization::from_contract(operation.contract_id.as_str())
                == Some(InputRealization::SpaceButton)
        })
        .collect();
    assert_eq!(inputs.len(), 2);
    assert_ne!(inputs[0].node, inputs[1].node);
    let mut admission = KeyboardRequestAdmission::prepare_body(
        &advertisement,
        &plan,
        &prospective,
        &identities,
        &operations,
    )
    .unwrap();
    admission
        .confirm_body_play(&advertisement, &prospective)
        .unwrap();
    for operation in inputs {
        admission
            .accept(InputRealization::SpaceButton, request(operation))
            .unwrap();
        let descriptor = admission
            .descriptors
            .iter()
            .find(|descriptor| descriptor.operation.node == operation.node)
            .unwrap();
        let partition = identities
            .iter()
            .find(|identity| identity.placement_for_node(operation.node).is_some())
            .unwrap();
        assert_eq!(descriptor.input_plan, partition.plan_id);
        assert!(matches!(descriptor.state, RequestAdmission::Current(_)));
    }
    let mut swapped = identities.clone();
    swapped.swap(0, 1);
    assert!(KeyboardRequestAdmission::prepare_body(
        &advertisement,
        &plan,
        &prospective,
        &swapped,
        &operations
    )
    .is_err());
}

#[test]
fn ordinary_body_entry_issues_origin_and_delivers_input_before_cancellation() {
    use crate::{
        body_execution::BodyRunRequest,
        hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll},
        RunControl, RunControlRequestId, TimerAdapter,
    };
    struct Clock;
    impl TimerAdapter for Clock {
        fn wait(&mut self, _: std::time::Duration) {}
        fn monotonic_now_ms(&mut self) -> Option<u64> {
            Some(0)
        }
    }
    struct Keys {
        next: u8,
        idle: u8,
        control: RunControl,
    }
    impl HostedKeyboardAdapter for Keys {
        fn poll_next(&mut self) -> HostedKeyboardPoll {
            if self.next < 4 {
                let event = conduit_human::KeyEvent::decode(&[0x2c, self.next % 2, 0]).unwrap();
                self.next += 1;
                HostedKeyboardPoll::Event(event)
            } else if self.idle < 32 {
                self.idle += 1;
                HostedKeyboardPoll::Pending
            } else {
                self.control
                    .request_stop(RunControlRequestId::new("stop-body-input-origin").unwrap())
                    .unwrap();
                HostedKeyboardPoll::Pending
            }
        }
    }
    let (advertisement, plan, wake) = body_plan();
    let mut host = crate::StdHost::from_advertisement(advertisement).unwrap();
    let control = RunControl::default();
    let mut keys = Keys {
        next: 0,
        idle: 0,
        control: control.clone(),
    };
    let mut output = Vec::new();
    let report = host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: Some(&mut keys),
            },
            &mut output,
            &mut Clock,
        )
        .unwrap();
    assert!(report.play.validate_for(&plan));
    assert!(matches!(
        report.terminal,
        conduit_core::TerminalDisposition::Cancelled { .. }
    ));
    assert!(
        report.kernel_failure.is_none(),
        "{:?}",
        report.kernel_failure
    );
    assert_eq!(keys.next, 4);
    let output = String::from_utf8(output).unwrap();
    assert_eq!(
        output
            .lines()
            .filter_map(|line| line.strip_prefix("bool value="))
            .collect::<Vec<_>>(),
        ["true", "false", "true", "false"]
    );
}
