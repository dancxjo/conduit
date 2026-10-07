//! Explicit temporal contracts for the same finite authored feature recipe.
use super::*;
pub(super) fn feature_source(flow: bool) -> String {
    let prefix = "type FarganPeriod = U16 in 32..=255\n";
    if flow {
        format!(
            "{prefix}{}\n{}",
            include_str!("../../../speech/fargan_feature_policy_flow.conduit"),
            include_str!("../../../speech/fargan_feature_correlation_flow.conduit")
        )
    } else {
        format!(
            "{prefix}{}\n{}",
            include_str!("../../../speech/fargan_feature_policy.conduit"),
            include_str!("../../../speech/fargan_feature_correlation.conduit")
        )
    }
}
#[test]
fn closing_feature_recipe_preserves_every_finite_expression_program_with_explicit_flow_owners() {
    let (startup, profiles) = catalogs(true);
    let value =
        check_syntax_document(&parse_syntax_document(&feature_source(false)), &startup).unwrap();
    let flow =
        check_syntax_document(&parse_syntax_document(&feature_source(true)), &startup).unwrap();
    let mut programs = 0;
    for plot in &value.plots {
        let value = expand_canonical_plot_for_authoring(&value, &plot.name, &profiles).unwrap();
        let name = plot.name.replace("speech/fargan-", "speech/flow-fargan-");
        let flow = expand_canonical_plot_for_authoring(&flow, &name, &profiles).unwrap();
        let expression = |graph: &ExpandedAuthoringPlot| -> Option<PortableExpressionProgram> {
            if graph.expanded.gears.len() != 1 {
                return None;
            }
            let ConfigurationValue::Text(encoded) =
                &graph.expanded.gears[0].configuration.first()?.value
            else {
                return None;
            };
            PortableExpressionProgram::from_canonical_hex(encoded).ok()
        };
        if let Some(expected) = expression(&value) {
            assert_eq!(expression(&flow).unwrap(), expected, "{name}");
            programs += 1;
        }
    }
    assert!(programs >= 20, "all finite numerical Source laws compared");
    let graph =
        expand_canonical_plot_for_authoring(&flow, "speech/flow-fargan-feature-frame", &profiles)
            .unwrap();
    assert!(!graph.expanded.gears.is_empty());
    let resources = graph
        .front
        .inputs()
        .iter()
        .filter(|port| port.temporal == PortTemporal::Value)
        .map(|port| port.value_kind.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(resources.len(), 3);
    for gear in &graph.expanded.gears {
        for port in gear.inputs.iter().chain(&gear.outputs) {
            if port.temporal == PortTemporal::Value {
                assert!(
                    resources.contains(&port.value_kind),
                    "nonresource Value port in closing feature graph: {gear:?}"
                );
            } else {
                assert_eq!(
                    port.temporal,
                    PortTemporal::Flow { closes: true },
                    "{gear:?}"
                );
            }
        }
    }
    eprintln!("explicit closing feature frame: {}nodes/{}cords; {programs} finite expression programs unchanged",graph.expanded.gears.len(),graph.expanded.connections.len());
}

pub(super) fn prepare_feedback() -> (
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
    std::collections::BTreeMap<String, String>,
) {
    use std::{collections::BTreeMap, sync::Arc};
    let mut context = prepared_epoch_profiles_with_capacity(true);
    let mut seeded = conduitos::seeded_state::SeededStateOperationFactory::default();
    let definition = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../speech/fargan_epoch_feedback.conduit")
        + "\n"
        + include_str!("../../../speech/fargan_feature_epoch_contracts.conduit");
    let mut ids = BTreeMap::new();
    let mut native = BTreeMap::new();
    for (key, name) in [
        ("INPUT", "FarganFeatureInputEpoch"),
        ("PROPOSAL", "FarganFeatureProposalEpoch"),
        ("PENDING", "FarganFeaturePendingState"),
        ("STATE", "FarganFeatureEpochFeedback"),
        ("EVENT", "FarganFeaturePcmEpoch"),
    ] {
        let profile = Arc::new(PreparedNativeProfile::check_definition(&definition, name).unwrap());
        profile
            .install(&mut context.startup, &mut context.profiles, true)
            .unwrap();
        ids.insert(
            format!("__FEATURE_{key}_NATIVE__"),
            profile.kind_identity(true),
        );
        native.insert(key, profile.value_type().clone());
        context.native.push(profile);
    }
    let state = &native["STATE"];
    let event = &native["EVENT"];
    let cell = conduit_semantic_catalog::install_seeded_state_flow_specialized_kind(
        &shape_contract(state),
        state,
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    assert_eq!(
        seeded
            .install_flow_specialized_frame16k(&shape_contract(state), state)
            .unwrap()
            .kind_id,
        cell
    );
    ids.insert("__FEATURE_CELL__".into(), cell.as_str().into());
    let zip = conduit_semantic_catalog::install_flow_zip_feedback_specialized_kind(
        &shape_contract(state),
        state,
        &shape_contract(event),
        event,
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    assert_eq!(
        context
            .zip
            .install_feedback_specialized(
                &shape_contract(state),
                state,
                &shape_contract(event),
                event
            )
            .unwrap()
            .kind_id,
        zip
    );
    ids.insert("__FEATURE_ZIP__".into(), zip.as_str().into());
    let paired = PreparedTypedTuplePairEncoder::new(
        state.clone(),
        maximum_prepared_transport_value_bytes(state).unwrap(),
        event.clone(),
        maximum_prepared_transport_value_bytes(event).unwrap(),
    )
    .unwrap();
    let weak = Arc::new(PreparedNominalWeakening::prepare(paired.value_type().clone()).unwrap());
    weak.install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert("__FEATURE_INPUT_WEAK__".into(), weak.kind_identity(true));
    let guard =
        conduit_ai::fixed_numeric_guard::FixedGuardProfile::prepare(weak.output_type().clone())
            .unwrap();
    guard
        .install(&mut context.startup, &mut context.profiles)
        .unwrap();
    ids.insert(
        "__FEATURE_INPUT_GUARD__".into(),
        guard.contract().unwrap().kind_id.as_str().into(),
    );
    context.guards.push(guard);
    context.weakening.push(weak);
    (context, seeded, ids)
}

#[test]
fn feature_feedback_preparation_retains_exact_separate_state_and_pcm_event() {
    let (context, _, ids) = prepare_feedback();
    for key in ["INPUT", "PROPOSAL", "PENDING", "STATE", "EVENT"] {
        assert!(ids.contains_key(&format!("__FEATURE_{key}_NATIVE__")));
    }
    assert!(context
        .native
        .iter()
        .any(|profile| profile.kind_identity(true) == ids["__FEATURE_STATE_NATIVE__"]));
}

fn feedback_entry_source(ids: &std::collections::BTreeMap<String, String>) -> String {
    format!("with {weak}/result as FeatureFeedbackPair\nwith {native}/candidate as FeatureInputCandidate\nplot speech/flow-fargan-feature-input-matches (\nvalue: FeatureFeedbackPair...| >> result: Boolean...|\n) = ((.item-00000.next_epoch == .item-00001.epoch) && (.item-00001.epoch < 18446744073709551615))\nplot speech/flow-fargan-feature-input-candidate (\nvalue: FeatureFeedbackPair...| >> result: FeatureInputCandidate...|\n) = {{ samples: .item-00001.samples, history: .item-00000.history, previous_raw: .item-00000.previous_raw, previous_normalized: .item-00000.previous_normalized, period: .item-00001.period, epoch: .item-00001.epoch }}\n",weak=ids["__FEATURE_INPUT_WEAK__"], native=ids["__FEATURE_INPUT_NATIVE__"])
}
#[test]
fn feature_feedback_entry_constructs_distinct_raw_candidate_from_exact_retained_pair() {
    let (context, _, ids) = prepare_feedback();
    let source = feedback_entry_source(&ids);
    let document =
        check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    for name in [
        "speech/flow-fargan-feature-input-matches",
        "speech/flow-fargan-feature-input-candidate",
    ] {
        let graph =
            expand_canonical_plot_for_authoring(&document, name, &context.profiles).unwrap();
        assert_eq!(graph.expanded.gears.len(), 1);
    }
}
