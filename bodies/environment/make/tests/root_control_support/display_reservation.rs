//! Production reservation policy; entropy, Boot and GPIO are CPU substitutes.
use super::*;
use conduit_environment_make::{DisplayOutputSource, PreparedDisplayOutput};
use conduit_environment_runtime::display_output::*;
#[test]
fn output_reservation_requires_admitted_scope_and_preserves_sequence_on_refusal() {
    let source = DisplayOutputSource::compile().unwrap();
    let (host, mut grant, _software) = display_fixture::fixture(&source);
    grant.grant_id = "grant/esp32/tm1637-write/1".into();
    let prepared = PreparedDisplayOutput::prepare(
        &source,
        &host,
        &["fixture/base".into(), "conduit.base/local@1".into()],
        &[grant],
    )
    .unwrap();
    let fragment = &prepared.plan().fragments[0];
    let material = prepared.display_layout().unwrap();
    let generated = conduit_embedded_build::generate_fore_native_plan(
        prepared.plan(),
        &fragment.fragment_id,
        conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 98304,
        },
        2,
    )
    .unwrap();
    let maxima = conduit_environment_make::DisplayLayoutMaterial::maxima();
    let bytes = generated.encode(maxima).unwrap();
    let records = c::AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let computations =
        conduit_embedded_build::assigned_computation_requirements(generated.kernel()).unwrap();
    let calls: Vec<_> = generated
        .kernel()
        .host_calls
        .iter()
        .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
        .collect();
    let resources: Vec<_> = generated
        .kernel()
        .resources
        .iter()
        .map(|r| r.resource)
        .collect();
    let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
    counts.copy_from_slice(&bytes[80..93]);
    let write = fragment
        .placements
        .iter()
        .position(|p| p.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND)
        .unwrap();
    let profile = AssignedDisplayProfile {
        source: material.source(),
        layout: conduit_environment_runtime::assigned::ResidentLayout {
            records: material.records(),
            counts,
        },
        computations: c::AssignedComputationProfile {
            maximum_text_bytes: u16::MAX,
            maximum_total_text_bytes: u32::MAX,
            requirements: &computations,
        },
        fore: material.fore(),
        host_calls: &calls,
        resources: &resources,
        write: conduit_kernel::NodeId(write as u16),
        sign_items: prepared.lowered().sign_items,
        sign_bytes: prepared.lowered().sign_bytes,
    };
    let assignment = ValidatedAssignedDisplay::admit(
        AssignedDisplayInput {
            header: &bytes[..125],
            records: &records,
            maxima,
        },
        &profile,
        AssignedDisplayScope {
            host: &host.host_id,
            boot: &host.boot_id,
            generation: host.offer_generation,
        },
    )
    .unwrap();
    for mode in [0, 4] {
        fixtures::scope_mode(mode);
        let mut foreign = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
        let expected = c::bind_active_play(&fragment.plan_id, &host.host_id, &host.boot_id, 1);
        assert!(foreign
            .reserve_display_play_matching(
                &assignment,
                &fragment.plan_id,
                &fragment.fragment_id,
                expected.active_play_id.as_str()
            )
            .is_err());
        assert_eq!(foreign.next_play_sequence(), Some(1));
    }
    fixtures::scope_mode(3);
    let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
    fixtures::scope_mode(0);
    let expected = c::bind_active_play(&fragment.plan_id, &host.host_id, &host.boot_id, 1);
    for (plan, part, play) in [
        (
            c::PlanId::from("foreign"),
            fragment.fragment_id.clone(),
            expected.active_play_id.as_str(),
        ),
        (
            fragment.plan_id.clone(),
            c::FragmentId::from("foreign"),
            expected.active_play_id.as_str(),
        ),
        (
            fragment.plan_id.clone(),
            fragment.fragment_id.clone(),
            "foreign/play",
        ),
    ] {
        assert!(issuer
            .reserve_display_play_matching(&assignment, &plan, &part, play)
            .is_err());
        assert_eq!(issuer.next_play_sequence(), Some(1));
    }
    assert_eq!(
        issuer
            .reserve_display_play_matching(
                &assignment,
                &fragment.plan_id,
                &fragment.fragment_id,
                expected.active_play_id.as_str()
            )
            .unwrap(),
        expected
    );
    assert_eq!(issuer.next_play_sequence(), Some(2));
    assert!(issuer
        .reserve_display_play_matching(
            &assignment,
            &fragment.plan_id,
            &fragment.fragment_id,
            expected.active_play_id.as_str()
        )
        .is_err());
    assert_eq!(issuer.next_play_sequence(), Some(2));
    signed_reservation(&mut issuer, &assignment, fragment);
    possession(
        &mut issuer,
        &assignment,
        fragment,
        &bytes,
        &records,
        &profile,
        maxima,
    );
}

fn signed_reservation(
    issuer: &mut native_authority::RootIssuer,
    assignment: &ValidatedAssignedDisplay,
    fragment: &c::PlanFragment,
) {
    use ed25519_dalek::{Signer, SigningKey};
    use native_owner_control::{Operation, OwnerControl, Request, PROTOCOL};
    let key = SigningKey::from_bytes(&[11; 32]);
    let mut gate = OwnerControl::new(Some(key.verifying_key().to_bytes())).unwrap();
    let expected = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 2);
    for mode in 0..4 {
        let request = Request {
            protocol: PROTOCOL,
            sequence: mode + 1,
            operation: if mode == 0 {
                Operation::Start
            } else {
                Operation::Reserve
            },
            host: fragment.host_id.as_str(),
            boot: fragment.boot_id.as_str(),
            offer_generation: 1,
            body: "body/display",
            wake: "wake/display",
            body_plan: "body-plan/display",
            body_play: "body-play/display",
            plan: fragment.plan_id.as_str(),
            fragment: fragment.fragment_id.as_str(),
            fragment_play: if mode == 2 {
                "wrong/play"
            } else {
                expected.active_play_id.as_str()
            },
            assignment_seal: if mode == 1 { [43; 32] } else { [42; 32] },
        };
        let signature = key.sign(&request.transcript().unwrap()).to_bytes();
        let token = gate
            .authenticate(
                request,
                &signature,
                fragment.host_id.as_str(),
                fragment.boot_id.as_str(),
                1,
            )
            .unwrap();
        let reserved = native_owner_reservation::OwnerReservation::reserve_display(
            token, assignment, &[42; 32], issuer,
        );
        if mode < 3 {
            assert!(reserved.is_err());
            assert_eq!(issuer.next_play_sequence(), Some(2));
        } else {
            let reserved = reserved.unwrap();
            assert_eq!(reserved.active, expected);
            assert_eq!(reserved.assignment_seal, [42; 32]);
            assert_eq!(reserved.body, "body/display");
            assert_eq!(issuer.next_play_sequence(), Some(3));
        }
    }
}

fn possession(
    issuer: &mut native_authority::RootIssuer,
    assignment: &ValidatedAssignedDisplay,
    fragment: &c::PlanFragment,
    bytes: &[u8],
    records: &[c::AssignedRecord<'_>],
    profile: &AssignedDisplayProfile<'_>,
    maxima: c::AssignedPlanMaxima,
) {
    use conduit_plan_lowering::{
        selected_assigned_operation::AssignedOperationInput,
        selected_assigned_operation::{
            SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
        },
    };
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 2);
    let owner = display_fixture::ready(&fragment.host_id, &fragment.boot_id);
    let contract = conduit_tm1637_wire::WriteContract::prepare().unwrap();
    let required = c::AssignedPlanRequirements {
        host: assignment.identity().host,
        boot: assignment.identity().boot,
        host_calls: profile.host_calls,
        resources: profile.resources,
        remote_bindings: &[],
    };
    let selected = || SelectedAssignedForeOperationPlan {
        operation: SelectedAssignedOperationPlan {
            input: AssignedOperationInput::Records {
                header: &bytes[..125],
                records,
            },
            maxima,
            requirements: required,
            node: profile.write,
            computations: profile.computations,
            fragment_id: &fragment.fragment_id,
            active: &active,
        },
        fore: profile.fore,
    };
    assert!(issuer
        .issue_assigned_display(&owner, assignment, selected(), 0, &contract)
        .is_err());
    let mut missing = selected();
    missing.fore = &[];
    assert!(issuer
        .issue_assigned_display(&owner, assignment, missing, 2, &contract)
        .is_err());
    let mut wrong_node = selected();
    wrong_node.operation.node = conduit_kernel::NodeId((profile.write.0 + 1) % 4);
    assert!(issuer
        .issue_assigned_display(&owner, assignment, wrong_node, 2, &contract)
        .is_err());
    let (table, handle, claim) = issuer
        .issue_assigned_display(&owner, assignment, selected(), 2, &contract)
        .unwrap();
    assert!(issuer
        .issue_assigned_display(&owner, assignment, selected(), 2, &contract)
        .is_err());
    let inspection = table.inspections().next().unwrap();
    assert_eq!(inspection.scope.maximum_operations, 2);
    assert_eq!(inspection.scope.maximum_in_flight, 1);
    assert_eq!(inspection.completed_operations, 0);
    assert_eq!(inspection.scope.active_play_id, active.active_play_id);
    let mut owner = Some(owner);
    conduit_tm1637_wire::installation::ReadyWriteBase::bind_assigned_fore_retained(
        &mut owner,
        table,
        handle,
        claim,
        &contract,
        selected(),
    )
    .unwrap();
    assert!(owner.is_none());
}
