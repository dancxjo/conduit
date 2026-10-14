//! Authored scan and retained child kernels in explicit augmented software Host/
//! registry fixtures. This is not installed Body, input, or controller proof.
use super::*;
use conduit_composite::{BoundedScanAdmission, BoundedScanState};
use conduit_core::{
    port_id, prepare_plan_on_hosts, ConnectionTrack, HostAdvertisement, PlannedActivationEntry,
    PortDirection, ValuePayload,
};
use conduit_face::{ParsecMapCommand, ParsecMapTransitionResult, PARSEC_MAP_FRAME_BYTES};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};
use std::{collections::BTreeMap, sync::Arc};

fn advertisement() -> HostAdvertisement {
    let mut host = crate::StdHost::new().advertisement().clone();
    let mut offers = required_parsec_source_offers().unwrap();
    offers.push(parsec_combine_offer().unwrap());
    offers.push(parsec_scan_offer(32).unwrap());
    for offer in offers {
        if !host
            .capabilities
            .iter()
            .any(|current| current.capability_id == offer.capability_id)
        {
            host.capabilities.push(offer);
        }
    }
    host.capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    host
}
pub(crate) fn authored_parsec_plan_on_host(
    advertisement: &HostAdvertisement,
) -> conduit_core::Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_parsec_scan_catalogs(&mut startup, &mut profile).unwrap();
    let checked =
        check_syntax_document(&parse_syntax_document(PARSEC_SCAN_SOURCE), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "parsec/main", &profile).unwrap();
    let hosts = [advertisement.clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let bounds = BTreeMap::from([
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("commands"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
            },
        ),
        (
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("states"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
            },
        ),
    ]);
    conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &expanded,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &bounds,
    )
    .unwrap()
}
fn activation(plan: &conduit_core::Plan) -> &conduit_core::PlannedScanActivation {
    let PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
        panic!("actual authored scan");
    };
    scan
}
fn command(action: &str, keyboard: bool, revision: u64) -> ValuePayload {
    ValuePayload {
        value_kind: parsec_command_kind().unwrap(),
        encoded: ParsecMapCommand::new(action.into(), revision, keyboard)
            .unwrap()
            .into_structured()
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    }
}
fn output() -> ValuePayload {
    ValuePayload {
        value_kind: parsec_result_kind().unwrap(),
        encoded: Vec::with_capacity(PARSEC_MAP_FRAME_BYTES),
    }
}
fn installed(
    advertisement: &HostAdvertisement,
    plan: &conduit_core::Plan,
) -> conduit_composite::BoundedScanActivationHost {
    let sources = Arc::new(ParsecSourcePlans::prepare(advertisement).unwrap());
    let mut host = StdActivationHost::new(
        sources.host_identity(),
        parsec_child_registry(sources).unwrap(),
    );
    let mut prepared = prepare_plan_on_hosts(plan, &mut [&mut host]).unwrap();
    let mut scan = install_pure_parsec_scan(
        plan,
        &mut prepared,
        &activation(plan).activation_id,
        &mut host,
    )
    .unwrap();
    scan.bind_parent_play(&conduit_core::ActivePlayId::from("parsec/parent-test-play"))
        .unwrap();
    scan
}
fn advance(
    scan: &mut conduit_composite::BoundedScanActivationHost,
    input: &ValuePayload,
    output: &mut ValuePayload,
) {
    assert_eq!(scan.admit(input).unwrap(), BoundedScanAdmission::Accepted);
    for _ in 0..64 {
        if matches!(scan.step().unwrap(), BoundedScanState::OutputReady) {
            break;
        }
    }
    assert!(scan.output_into(output).unwrap());
    scan.complete_output().unwrap();
}
fn decoded(bytes: &[u8]) -> ParsecMapTransitionResult {
    ParsecMapTransitionResult::from_structured(
        conduit_core::StructuredInfoValue::from_canonical_bytes(bytes).unwrap(),
    )
    .unwrap()
}

#[test]
fn actual_child_scan_retains_source_phase_choice_revision_and_play_signs_without_allocation() {
    let advertisement = advertisement();
    let plan = authored_parsec_plan_on_host(&advertisement);
    pure_parsec_scan::exact_pure_child(&plan, &activation(&plan).activation_id).unwrap();
    let mut scan = installed(&advertisement, &plan);
    let inputs = [
        command("game/start", false, 1),
        command("game/shoot", false, 2),
        command("game/remap", true, 2),
        command("game/shoot", false, 2),
        command("game/start", true, 3),
        command("game/shoot", false, 3),
    ];
    let mut output = output();
    let capacities = (scan.allocation_capacities(), scan.storage_capacities());
    let mut states = Vec::with_capacity(inputs.len());
    // Copy into already admitted buffers; decoding/assertions occur after tracking.
    let mut captures = (0..inputs.len())
        .map(|_| Vec::with_capacity(PARSEC_MAP_FRAME_BYTES))
        .collect::<Vec<_>>();
    let allocations = crate::allocation_probe::begin();
    for (input, capture) in inputs.iter().zip(&mut captures) {
        advance(&mut scan, input, &mut output);
        capture.extend_from_slice(&output.encoded);
    }
    assert_eq!(allocations.finish(), 0);
    assert_eq!(
        (scan.allocation_capacities(), scan.storage_capacities()),
        capacities
    );
    for capture in captures {
        states.push(decoded(&capture));
    }
    for (index, accepted, started, keyboard, revision) in [
        (0, true, true, false, 2),
        (1, true, true, false, 2),
        (2, true, true, true, 3),
        (3, false, true, true, 3),
        (4, false, true, true, 3),
        (5, true, true, true, 3),
    ] {
        assert_eq!(*states[index].accepted(), accepted);
        assert_eq!(*states[index].state().started(), started);
        assert_eq!(*states[index].state().keyboard(), keyboard);
        assert_eq!(*states[index].state().mapping_revision(), revision);
    }
    let receipts = scan.child_sign_receipts().unwrap();
    assert_eq!(receipts.len(), inputs.len());
    assert!(receipts
        .iter()
        .all(
            |receipt| receipt.parent_active_play_id.as_str() == "parsec/parent-test-play"
                && receipt.selected_plan_id == activation(&plan).selected_plan_id
                && !receipt.events.is_empty()
        ));
}

#[test]
fn pressure_cancellation_revokes_pending_child_and_late_input_without_growing_storage() {
    let advertisement = advertisement();
    let plan = authored_parsec_plan_on_host(&advertisement);
    let mut scan = installed(&advertisement, &plan);
    let input = command("game/start", false, 1);
    let capacities = (scan.allocation_capacities(), scan.storage_capacities());
    let allocations = crate::allocation_probe::begin();
    assert_eq!(scan.admit(&input).unwrap(), BoundedScanAdmission::Accepted);
    assert_eq!(scan.admit(&input).unwrap(), BoundedScanAdmission::Accepted);
    assert_eq!(scan.admit(&input).unwrap(), BoundedScanAdmission::Full);
    scan.step().unwrap();
    scan.cancel().unwrap();
    assert_eq!(scan.step().unwrap(), &BoundedScanState::Cancelled);
    assert!(scan.admit(&input).is_err());
    assert_eq!(allocations.finish(), 0);
    assert_eq!(
        (scan.allocation_capacities(), scan.storage_capacities()),
        capacities
    );
}

#[test]
fn source_preparation_requires_actual_offers_and_factory_refuses_changed_boot_or_back() {
    let advertisement = advertisement();
    let plan = authored_parsec_plan_on_host(&advertisement);
    let mut missing = advertisement.clone();
    let required = required_parsec_source_offers().unwrap();
    missing.capabilities.retain(|offer| {
        !required
            .iter()
            .any(|required| required.capability_id == offer.capability_id)
    });
    assert!(ParsecSourcePlans::prepare(&missing).is_err());
    let sources = Arc::new(ParsecSourcePlans::prepare(&advertisement).unwrap());
    let factory = ParsecCombineFactory::new(sources);
    let child =
        pure_parsec_scan::exact_pure_child(&plan, &activation(&plan).activation_id).unwrap();
    factory.validate(child).unwrap();
    let mut changed = child.clone();
    changed.boot_id = "different/boot".into();
    assert!(factory.validate(&changed).is_err());
    changed = child.clone();
    changed.offer_generation.0 += 1;
    assert!(factory.validate(&changed).is_err());
    changed = child.clone();
    changed.artifact_id = "different/artifact".into();
    assert!(factory.validate(&changed).is_err());
    let mut changed_plan = plan.clone();
    changed_plan.plan_id = "public/forged-plan-id".into();
    assert!(
        pure_parsec_scan::exact_pure_child(&changed_plan, &activation(&plan).activation_id)
            .is_err()
    );
}
