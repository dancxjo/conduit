use super::*;

const FORM: &str = "form typed_gate {\n script: conduit-test/gate-script\n latest: state/latest\n split: flow/tee\n gate: flow/gate(maximum-enable-updates = 3)\n gated: conduit-test/scalar-sink(expected = 1)\n slow: conduit-test/slow-scalar-sink\n script.scalar > latest.in\n latest.out > split.in\n split.left > gate.in\n script.enable > gate.enable\n gate.out > gated.in\n split.right > slow.in\n}\n";

#[test]
fn latest_tee_and_gate_run_together_with_closed_open_closed_and_uneven_pressure() {
    let mut host = host("typed-gate-host");
    let form = parse(FORM, &installed_std::test_catalog()).expect("typed gate Form parses");
    let hosts = [host.advertisement().clone()];
    assert_gate_face_is_advertised(&form, &hosts[0]);
    let placements = default_placements(&form, &hosts).expect("typed gate placements resolve");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("typed gate plans with capacity-one Cords");
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.placements.len(), 6);
    assert_eq!(fragment.connections.len(), 6);
    assert!(fragment.connections.iter().all(|cord| {
        cord.item_capacity == 1 && cord.byte_capacity == conduit_core::SCALAR_ENCODED_LEN as u32
    }));

    let gate = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::GATE_KIND)
        .expect("gate placement exists");
    assert_eq!(
        gate.kind_contract_revision.as_str(),
        conduit_std_catalog::FLOW_GATE_SCALAR_CONTRACT_REVISION
    );
    assert_eq!(
        gate.inputs[0].value_kind.as_str(),
        conduit_core::SCALAR_INFO_ID
    );
    assert_eq!(
        gate.inputs[1].value_kind.as_str(),
        conduit_core::BOOL_INFO_ID
    );
    assert_eq!(gate.host_operations.len(), 1);
    assert_eq!(
        gate.host_operations[0].contract_id.as_str(),
        conduit_std_catalog::FLOW_GATE_BOOL_HOST_OPERATION_CONTRACT
    );

    let mut output = Vec::with_capacity(2_048);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(9),
    };
    let report = host
        .run_fragment_to(fragment.clone(), &mut output, &mut timer)
        .expect("latest/tee/gate execute through the production kernel");
    assert_eq!(timer.waits, vec![Duration::ZERO; 9]);
    let output = String::from_utf8(output).expect("gate report is UTF-8");
    assert!(output.contains("/latest kind=state/latest"));
    assert!(output.contains("/split kind=flow/tee"));
    assert!(output.contains("/gate kind=flow/gate"));
    assert!(output.contains(" complete\n"));
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn gate_zero_capacity_and_mutated_decoder_identity_fail_before_play() {
    let baseline_host = host("gate-negative-host");
    let form = parse(FORM, &installed_std::test_catalog()).expect("typed gate Form parses");
    let hosts = [baseline_host.advertisement().clone()];
    assert_gate_face_is_advertised(&form, &hosts[0]);
    let placements = default_placements(&form, &hosts).expect("typed gate placements resolve");
    assert!(plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 0,
            connection_byte_capacity: conduit_core::SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .is_err());

    let mut fragment = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("baseline gate plan exists")
    .fragments
    .remove(0);
    fragment
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::GATE_KIND)
        .expect("gate placement exists")
        .host_operations[0]
        .contract_id = conduit_core::HostOperationContractId::from("mutated/decode-bool");

    let mut host = baseline_host;
    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    let error = host
        .run_fragment_to(fragment, &mut output, &mut timer)
        .expect_err("mutated gate decoder identity must fail before Play");
    assert_eq!(error, "kernel preparation lowering: InvalidFragment");
    assert!(timer.waits.is_empty());
}

fn assert_gate_face_is_advertised(
    form: &conduit_form::CheckedForm,
    host: &conduit_core::HostAdvertisement,
) {
    let gear = form
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_std_catalog::GATE_KIND)
        .expect("checked gate Gear exists");
    let offer = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_std_catalog::GATE_KIND)
        .expect("reference std Host advertises gate");
    assert_eq!(offer.checked_face(), gear.checked_face());
}
