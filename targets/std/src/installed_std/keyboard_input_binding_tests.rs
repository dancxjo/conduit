//! Actual checked/planned input descriptors; deterministic mechanism proof only.
use super::*;
use conduit_core::{bind_active_play, resource_offer, INPUT_RESOURCE_CLASS};
use conduit_kernel::{BoundedValueRef, HostCallId, NodeId, RequestId, ValueRef};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

pub(super) fn planned_plan() -> (HostAdvertisement, conduit_core::Plan) {
    planned_plan_named("button_across_room")
}

pub(super) fn planned_plan_named(name: &str) -> (HostAdvertisement, conduit_core::Plan) {
    let mut advertisement = crate::StdHost::new().advertisement().clone();
    advertisement.capabilities = vec![
        conduit_std_offers::button::offer(),
        conduit_std_offers::button::mapper_offer(),
        conduit_std_offers::button::indicator_offer(),
    ];
    advertisement
        .resources
        .push(resource_offer("proof/keyboard", INPUT_RESOURCE_CLASS, 1));
    advertisement.resources.sort();
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    planned_plan_on_host(name, &advertisement)
}

pub(super) fn planned_plan_on_host(
    name: &str,
    advertisement: &HostAdvertisement,
) -> (HostAdvertisement, conduit_core::Plan) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)
        .unwrap();
    let source = include_str!("../../../../plots/button-across-room/main.conduit")
        .replace("button_across_room", name);
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, name, &profile).unwrap();
    let advertisement = advertisement.clone();
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let limits = expanded
        .connections
        .iter()
        .map(|connection| {
            (
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: if connection.value_kind.as_str() == conduit_core::BOOL_INFO_ID {
                        1
                    } else {
                        conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES
                    },
                },
            )
        })
        .collect();
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &Default::default(),
            line_candidates: &Default::default(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
    .unwrap();
    (advertisement, plan)
}

fn planned() -> (
    HostAdvertisement,
    PlanFragment,
    KernelIdentityMap,
    Vec<LoweredHostCall>,
    ActivePlayIdentity,
) {
    let (advertisement, plan) = planned_plan();
    let fragment = plan.fragments[0].clone();
    let lowered =
        super::super::preparation::lower_fragment_with_continuity(&fragment, false).unwrap();
    let play = bind_active_play(
        &fragment.plan_id,
        &advertisement.host_id,
        &advertisement.boot_id,
        0,
    );
    (
        advertisement,
        fragment,
        lowered.identity,
        lowered.host_calls,
        play,
    )
}

fn request(operation: &LoweredHostCall, sequence: u32) -> HostCallRequest {
    HostCallRequest {
        node: operation.node,
        call: operation.call,
        request: RequestId(sequence),
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
fn prepared_descriptors_do_not_admit_input_and_wrong_requests_revoke() {
    let (advertisement, fragment, identity, operations, play) = planned();
    let operation = operations
        .iter()
        .find(|operation| {
            InputRealization::from_contract(operation.contract_id.as_str())
                == Some(InputRealization::SpaceButton)
        })
        .unwrap();
    for wrong in [NodeId(operation.node.0 + 1), operation.node] {
        let mut admission = KeyboardRequestAdmission::prepare(
            &advertisement,
            &fragment,
            &identity,
            &operations,
            &play,
        )
        .unwrap();
        assert!(!admission.is_current(InputRealization::SpaceButton));
        let mut input = request(operation, 0);
        input.node = wrong;
        if wrong == operation.node {
            input.call = HostCallId(operation.call.0 + 1);
        }
        assert!(admission
            .accept(InputRealization::SpaceButton, input)
            .is_err());
        assert!(admission
            .accept(InputRealization::SpaceButton, request(operation, 1))
            .is_err());
        assert!(!admission.is_current(InputRealization::SpaceButton));
    }
}

#[test]
fn current_input_retains_origin_between_requests_and_never_survives_revocation_or_replay() {
    let (advertisement, fragment, identity, operations, play) = planned();
    let operation = operations
        .iter()
        .find(|operation| {
            InputRealization::from_contract(operation.contract_id.as_str())
                == Some(InputRealization::SpaceButton)
        })
        .unwrap();
    let mut admission =
        KeyboardRequestAdmission::prepare(&advertisement, &fragment, &identity, &operations, &play)
            .unwrap();
    admission
        .accept(InputRealization::SpaceButton, request(operation, 0))
        .unwrap();
    assert!(admission.is_current(InputRealization::SpaceButton));
    admission
        .accept(InputRealization::SpaceButton, request(operation, 1))
        .unwrap();
    assert!(admission.is_current(InputRealization::SpaceButton));
    assert!(admission
        .accept(InputRealization::SpaceButton, request(operation, 1))
        .is_err());
    assert!(!admission.is_current(InputRealization::SpaceButton));
    admission.revoke();
    assert!(admission
        .accept(InputRealization::SpaceButton, request(operation, 2))
        .is_err());
}

#[test]
fn stale_boot_offer_generation_play_or_changed_realization_refuses_preparation() {
    let (advertisement, fragment, identity, operations, play) = planned();
    let mut changed = advertisement.clone();
    changed.boot_id = "replacement/boot".into();
    assert!(
        KeyboardRequestAdmission::prepare(&changed, &fragment, &identity, &operations, &play)
            .is_err()
    );
    changed = advertisement.clone();
    changed.offer_generation.0 += 1;
    assert!(
        KeyboardRequestAdmission::prepare(&changed, &fragment, &identity, &operations, &play)
            .is_err()
    );
    let mut changed_play = play.clone();
    changed_play.plan_id = "different/plan".into();
    assert!(KeyboardRequestAdmission::prepare(
        &advertisement,
        &fragment,
        &identity,
        &operations,
        &changed_play
    )
    .is_err());
    let mut changed_fragment = fragment.clone();
    let source = changed_fragment
        .placements
        .iter_mut()
        .find(|placement| {
            placement.implementation_id.as_str() == conduit_std_offers::button::IMPLEMENTATION
        })
        .unwrap();
    source.implementation_id = "forged/back".into();
    assert!(KeyboardRequestAdmission::prepare(
        &advertisement,
        &changed_fragment,
        &identity,
        &operations,
        &play
    )
    .is_err());
    let mut changed_operations = operations.clone();
    let operation = changed_operations
        .iter_mut()
        .find(|operation| InputRealization::from_contract(operation.contract_id.as_str()).is_some())
        .unwrap();
    operation.binding.maximum_output_bytes += 1;
    assert!(KeyboardRequestAdmission::prepare(
        &advertisement,
        &fragment,
        &identity,
        &changed_operations,
        &play
    )
    .is_err());
}

struct Pending;
impl crate::hosted_keyboard::HostedKeyboardAdapter for Pending {
    fn poll_next(&mut self) -> crate::hosted_keyboard::HostedKeyboardPoll {
        crate::hosted_keyboard::HostedKeyboardPoll::Pending
    }
}

#[test]
fn adapter_existence_is_not_origin_and_cancel_or_rejected_request_revokes() {
    use super::super::keyboard_input_host::{InputRequestKind, KeyboardInputHost};
    let (advertisement, fragment, identity, operations, play) = planned();
    let operation = operations
        .iter()
        .find(|operation| {
            InputRealization::from_contract(operation.contract_id.as_str())
                == Some(InputRealization::SpaceButton)
        })
        .unwrap();
    for missing in [true, false] {
        let mut adapter = Pending;
        let mut host = KeyboardInputHost::new(
            if missing { None } else { Some(&mut adapter) },
            operations.iter().map(|op| &op.contract_id),
        );
        host.install_request_admission(
            KeyboardRequestAdmission::prepare(
                &advertisement,
                &fragment,
                &identity,
                &operations,
                &play,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!host.has_request_origin(InputRealization::SpaceButton));
        let accepted = host.accept(request(operation, 0), &[], InputRequestKind::SpaceButton);
        assert_eq!(accepted.is_ok(), !missing);
        assert_eq!(
            host.has_request_origin(InputRealization::SpaceButton),
            !missing
        );
        if !missing {
            assert!(host
                .accept(request(operation, 1), &[1], InputRequestKind::SpaceButton)
                .is_err());
            assert!(!host.has_request_origin(InputRealization::SpaceButton));
        }
        host.cancel();
        assert!(!host.has_request_origin(InputRealization::SpaceButton));
        assert!(host
            .accept(request(operation, 2), &[], InputRequestKind::SpaceButton)
            .is_err());
    }
}

#[test]
fn ordinary_installed_scheduler_accepts_multiple_actual_input_requests() {
    use crate::{
        hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll},
        RunControl, RunControlRequestId, TimerAdapter,
    };
    struct Timer;
    impl TimerAdapter for Timer {
        fn wait(&mut self, _: std::time::Duration) {}
    }
    struct Keys {
        next: u8,
        control: RunControl,
    }
    impl HostedKeyboardAdapter for Keys {
        fn poll_next(&mut self) -> HostedKeyboardPoll {
            if self.next < 4 {
                let event = conduit_human::KeyEvent::decode(&[0x2c, self.next % 2, 0]).unwrap();
                self.next += 1;
                HostedKeyboardPoll::Event(event)
            } else {
                self.control
                    .request_stop(RunControlRequestId::new("input-origin-proof-stop").unwrap())
                    .unwrap();
                HostedKeyboardPoll::Pending
            }
        }
    }
    let (advertisement, fragment, _, _, _) = planned();
    let mut host = crate::StdHost::from_advertisement(advertisement).unwrap();
    let control = RunControl::default();
    let mut keys = Keys {
        next: 0,
        control: control.clone(),
    };
    let mut output = Vec::new();
    let report = host
        .run_fragment_controlled_with_keyboard_to(
            fragment,
            &mut output,
            &mut Timer,
            &control,
            Some(&mut keys),
        )
        .unwrap();
    let output = String::from_utf8(output).unwrap();
    let states: Vec<_> = output
        .lines()
        .filter_map(|line| line.strip_prefix("bool value="))
        .collect();
    assert_eq!(states, ["true", "false", "true", "false"]);
    assert!(matches!(
        report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Cancelled { .. }
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
