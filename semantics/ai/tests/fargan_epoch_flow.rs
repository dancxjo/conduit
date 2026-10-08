#![cfg(feature = "kernel-operation-owners")]
#[path = "fargan_epoch_flow/allocation_probe.rs"]
mod allocation_probe;
#[path = "fargan_epoch_flow/committed_lineage.rs"]
mod committed_lineage;
#[path = "fargan_epoch_flow/conditioning_cycle.rs"]
mod conditioning_cycle;
#[path = "fargan_epoch_contracts.rs"]
mod declarations;
#[path = "fargan_epoch_flow/epoch_profiles.rs"]
mod epoch_profiles;
#[path = "fargan_epoch_flow/feature_cycle.rs"]
mod feature_cycle;
#[path = "fargan_epoch_flow/guest_fixture.rs"]
mod guest_fixture;
#[path = "fargan_epoch_flow/interface.rs"]
mod interface;
#[path = "fargan_epoch_flow/native_session.rs"]
mod native_session;
#[path = "fargan_epoch_flow/native_startup.rs"]
mod native_startup;
#[path = "fargan_epoch_flow/plan_artifact.rs"]
mod plan_artifact;
#[path = "fargan_epoch_flow/repeat_capacity.rs"]
mod repeat_capacity;
#[path = "fargan_epoch_flow/synthetic_resources.rs"]
mod synthetic_resources;
#[path = "fargan_epoch_flow/trace_cycle.rs"]
mod trace_cycle;
#[path = "fargan_epoch_flow/trace_observer.rs"]
mod trace_observer;
use epoch_profiles::*;
#[path = "fargan_epoch_flow/epoch_planning.rs"]
mod epoch_planning;
use epoch_planning::*;
#[path = "fargan_epoch_flow/signal_cycle_profiles.rs"]
mod signal_cycle_profiles;
use signal_cycle_profiles::*;
#[path = "fargan_epoch_flow/signal_cycle_planning.rs"]
mod signal_cycle_planning;
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_pair_catalog::*, native_profile::PreparedNativeProfile,
    nominal_weakening::PreparedNominalWeakening,
};
use conduit_core::*;
use conduit_plot::*;
use signal_cycle_planning::*;
#[test]
fn authored_epoch_imports_and_owners_match_exact_checked_profiles() {
    let context = prepared_epoch_profiles();
    let imports = context.imports;
    let kinds = context.kinds;
    let source = include_str!("../../speech/fargan_epoch_flow.conduit");
    for import in imports.lines() {
        assert!(source.lines().any(|line| line == import), "{import}");
    }
    for identity in kinds.values() {
        assert!(source.contains(identity), "{identity}");
    }
    assert!(!source.contains("__"));
}

#[test]
fn complete_four_phase_epoch_checks_and_expands_exact_closing_flow_owners() {
    let context = prepared_epoch_profiles();
    assert_eq!(context.native.len(), 6);
    assert_eq!(context.weakening.len(), 3);
    assert_eq!(context.guards.len(), 1);
    assert_eq!(context.zip.offers().count(), 0);
    assert_eq!(context.pairs.len(), 2);
    let startup = context.startup;
    let profiles = context.profiles;
    let source = epoch_source();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "speech/flow-fargan-float-epoch", &profiles)
            .unwrap();
    eprintln!(
        "complete epoch: {} nodes, {} cords",
        expanded.expanded.gears.len(),
        expanded.expanded.connections.len()
    );
    let mut multiplicities = std::collections::BTreeMap::new();
    for gear in &expanded.expanded.gears {
        *multiplicities
            .entry(gear.kind_id.as_str())
            .or_insert(0usize) += 1;
    }
    for (kind, count) in multiplicities.iter().filter(|(_, count)| **count > 16) {
        eprintln!("epoch multiplicity: {kind} = {count}");
    }
    assert!(expanded.expanded.gears.len() > 600);
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "numeric/flow-dense128x40")
            .count(),
        4
    );
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|gear| gear
                .kind_id
                .as_str()
                .starts_with("structure/flow-native-profile/"))
            .count(),
        6
    );
    for gear in &expanded.expanded.gears {
        for port in gear.inputs.iter().chain(&gear.outputs) {
            assert_eq!(
                port.temporal,
                if ["weights", "bias"].contains(&port.port_id.as_str()) {
                    PortTemporal::Value
                } else {
                    PortTemporal::Flow { closes: true }
                },
                "{}:{}",
                gear.kind_id.as_str(),
                port.port_id.as_str()
            );
        }
    }
}

fn unbound_epoch_source() -> String {
    format!(
        "{}\n{}\n{}",
        include_str!("../../speech/fargan_epoch_flow.conduit"),
        (declarations::exact_epoch_declarations()
            + "\n"
            + &declarations::declarations_only(include_str!(
                "../../speech/fargan_pitch_history.conduit"
            )))
            .lines()
            .filter(|line| !line.starts_with("type Numeric"))
            .collect::<Vec<_>>()
            .join("\n"),
        [
            include_str!("../../speech/fargan_signal_flow.conduit"),
            include_str!("../../speech/fargan_pitch_history_flow.conduit"),
            include_str!("../../speech/fargan_subframe_flow.conduit"),
            include_str!("../../speech/fargan_epoch_projections.conduit"),
            include_str!("../../speech/fargan_epoch_policy.conduit"),
            include_str!("../../speech/fargan_epoch_anchor.conduit"),
            include_str!("../../speech/fargan_epoch_merges.conduit"),
        ]
        .join("\n")
    )
}
fn epoch_source() -> String {
    let mut source = unbound_epoch_source();
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let selected=format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    source = source.replace(
        "selected: FarganModelFrameAnchor",
        &format!("selected: FarganModelFrameAnchor = {selected}"),
    );
    source
}

#[test]
fn default_numeric_capacity_refuses_four_simultaneous_epoch_phases() {
    match prepared_epoch_plan(false) {
        Err(conduit_planner::PlannerError::UnknownCapability(kind)) => {
            assert!(matches!(
                kind.as_str(),
                "numeric/flow-add128"
                    | "numeric/flow-multiply128"
                    | "numeric/flow-scale40"
                    | "numeric/flow-sigmoid128"
                    | "numeric/flow-slice384x128"
                    | "numeric/flow-slice480x160"
            ));
        }
        Err(error) => panic!("unexpected preparation refusal: {error:?}"),
        Ok(_) => panic!("default sixteen-instance profile must refuse the four-phase graph"),
    }
}

#[test]
fn signal_feedback_and_condition_event_fit_exact_selected_pair_profile() {
    let definition = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../speech/fargan_epoch_feedback.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(&definition), &StartupCatalog::new()).unwrap();
    let find = |name| {
        checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    let state = find("FarganSignalEpochFeedback");
    let event = find("FarganSignalConditionEpoch");
    let pair = PreparedTypedTuplePairEncoder::new(
        state.clone(),
        maximum_prepared_transport_value_bytes(&state).unwrap(),
        event.clone(),
        maximum_prepared_transport_value_bytes(&event).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Signal cycle: state={}, event={}, pair={}",
        maximum_prepared_transport_value_bytes(&state).unwrap(),
        maximum_prepared_transport_value_bytes(&event).unwrap(),
        pair.maximum_bytes()
    );
    assert!(pair.maximum_bytes() as usize <= MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    assert_eq!(
        pair.maximum_bytes(),
        maximum_prepared_transport_value_bytes(pair.value_type()).unwrap()
    );
    let profile =
        PreparedNativeProfile::check_definition(&definition, "FarganSignalEpochFeedback").unwrap();
    assert_eq!(profile.value_type(), &state);
}

#[test]
fn signal_cycle_exact_specializations_retain_seed_feedback_and_full_admission() {
    let (context, seeded, ids) = prepared_signal_cycle_profiles();
    assert_eq!(seeded.offers().count(), 1);
    assert_eq!(context.zip.offers().count(), 1);
    assert_eq!(context.pairs.len(), 2);
    assert_eq!(context.native.len(), 7);
    assert_eq!(context.weakening.len(), 5);
    assert_eq!(context.guards.len(), 2);
    eprintln!(
        "signal cycle identities: {}",
        serde_json::to_string(&ids).unwrap()
    );
}

#[test]
fn authored_signal_feedback_cycle_checks_exact_epoch_guard_and_final_pcm_ack() {
    let (context, _, ids) = prepared_signal_cycle_profiles();
    let cycle = include_str!("../../speech/fargan_signal_cycle.conduit");
    for identity in ids.values() {
        assert!(cycle.contains(identity), "{identity}");
    }
    let imports = cycle
        .lines()
        .filter(|line| line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let body = cycle
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let selected=format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let body = body.replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {selected}\n"),
    );
    let source = format!(
        "{imports}\n{}\n{}\n{body}",
        epoch_source(),
        include_str!("../../speech/fargan_epoch_feedback.conduit")
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let matcher = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-signal-cycle-matches",
        &context.profiles,
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &matcher.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for (next, event, accepted) in [
        (0, 0, true),
        (7, 7, true),
        (7, 8, false),
        (u64::MAX - 1, u64::MAX - 1, true),
        (u64::MAX, u64::MAX, false),
    ] {
        let input = epoch_pair_fixture(&program.input_type, "", next, event)
            .canonical_bytes()
            .unwrap();
        let expected = InfoBool::new(accepted).encode();
        assert_eq!(program.evaluate(&input).unwrap(), expected);
        assert_eq!(prepared.evaluate(&input).unwrap(), expected);
    }
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-signal-cycle",
        &context.profiles,
    )
    .unwrap();
    eprintln!(
        "Signal epoch cycle: {} nodes, {} cords",
        expanded.expanded.gears.len(),
        expanded.expanded.connections.len()
    );
    assert!(expanded.expanded.gears.len() > 742);
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == ids["__SIGNAL_CELL__"])
            .count(),
        1
    );
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == ids["__SIGNAL_ZIP__"])
            .count(),
        1
    );
}

fn epoch_pair_fixture(
    ty: &StructuredInfoType,
    name: &str,
    next: u64,
    event: u64,
) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => StructuredInfoValue::nominal(
            ty.clone(),
            epoch_pair_fixture(representation, name, next, event),
        )
        .unwrap(),
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        epoch_pair_fixture(field.value_type(), field.name(), next, event),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/u64" => {
            StructuredInfoValue::leaf(
                ty.clone(),
                if name == "next_epoch" { next } else { event }
                    .to_le_bytes()
                    .to_vec(),
            )
            .unwrap()
        }
        _ => declarations::fixture_value(ty),
    }
}

#[test]
fn explicit_capacity64_epoch_plan_selects_exact_generic_owners() {
    let (plan, context) = prepared_epoch_plan(true).unwrap();
    assert_eq!(context.native.len(), 6);
    assert_eq!(context.weakening.len(), 3);
    assert_eq!(context.zip.offers().count(), 0);
    assert_eq!(context.pairs.len(), 2);
    assert!(plan.fragments[0].placements.len() > 742);
}

#[allow(dead_code)]
#[path = "fargan_signal_graph/shared.rs"]
mod fixtures;
#[path = "fargan_epoch_flow/runtime.rs"]
mod runtime;

#[test]
fn synthetic_epoch_anchor_matches_explicit_floating_startup() {
    let context = prepared_epoch_profiles();
    let checked =
        check_syntax_document(&parse_syntax_document(&epoch_source()), &context.startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-model-anchor-matches",
        &context.profiles,
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("Source matcher program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let input = declarations::fixture_value(&program.input_type)
        .canonical_bytes()
        .unwrap();
    assert_eq!(evaluator.evaluate(&input).unwrap(), [1]);
}

#[test]
fn conditioning_feedback_domains_preserve_exact_native_bounds() {
    let source = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../speech/fargan_epoch_feedback.conduit")
        + "\n"
        + include_str!("../../speech/fargan_conditioning_epoch_contracts.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    for name in [
        "FarganFeatureConditionEpoch",
        "FarganConditioningInputEpoch",
        "FarganConditioningPendingHistory",
        "FarganConditioningProposalEpoch",
    ] {
        let selected = checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap();
        let maximum = maximum_prepared_transport_value_bytes(&selected.value_type).unwrap();
        assert!(maximum <= 16384);
        eprintln!("conditioning domain {name}: exact maximum {maximum}B");
    }
}

#[test]
fn source_zero_continuation_startup_checks_four_sequential_subframes() {
    let context = prepared_epoch_profiles_with_capacity(true);
    let source = [
        include_str!("../../speech/fargan_conditioning.conduit"),
        include_str!("../../speech/fargan_signal.conduit"),
        include_str!("../../speech/fargan_pitch_history.conduit"),
        include_str!("../../speech/fargan_subframe.conduit"),
        include_str!("../../speech/fargan_zero_continuation.conduit"),
    ]
    .join("\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-zero-continuation-startup",
        &context.profiles,
    )
    .unwrap();
    let count = expanded
        .expanded
        .gears
        .iter()
        .filter(|g| g.kind_id.as_str() == "numeric/dense128x40")
        .count();
    assert_eq!(count, 4);
    assert!(expanded.expanded.gears.len() <= 1024);
    eprintln!("Source zero-continuation warm startup: {}nodes/{}cords; four explicit signal updates and reset",expanded.expanded.gears.len(),expanded.expanded.connections.len());
}

#[path = "fargan_epoch_flow/custody.rs"]
mod custody;

#[allow(dead_code)]
#[path = "fargan_signal_graph/state.rs"]
mod case_state;
