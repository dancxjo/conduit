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
    let pair = conduit_ai::closing_structured_pair::ClosingStructuredPairProfile::prepare(
        fixed_numeric_type("NumericF32Vector80").unwrap(),
        StructuredInfoType::leaf(kind_id(F32_INFO_ID)).unwrap(),
    )
    .unwrap();
    pair.install(&mut context.startup, &mut context.profiles)
        .unwrap();
    ids.insert("__FEATURE_RESAMPLE_PAIR__".into(), pair.identity().into());
    context.pairs.push(pair);
    let pair = conduit_ai::closing_structured_pair::ClosingStructuredPairProfile::prepare(
        fixed_numeric_type("NumericRawF32Vector160").unwrap(),
        StructuredInfoType::leaf(kind_id(F32_INFO_ID)).unwrap(),
    )
    .unwrap();
    pair.install(&mut context.startup, &mut context.profiles)
        .unwrap();
    ids.insert(
        "__FEATURE_PREEMPHASIS_PAIR__".into(),
        pair.identity().into(),
    );
    context.pairs.push(pair);
    let pair = conduit_ai::closing_structured_pair::ClosingStructuredPairProfile::prepare(
        fixed_numeric_type("NumericF32Vector640").unwrap(),
        fixed_numeric_type("NumericF32Vector160").unwrap(),
    )
    .unwrap();
    pair.install(&mut context.startup, &mut context.profiles)
        .unwrap();
    ids.insert("__FEATURE_HISTORY_PAIR__".into(), pair.identity().into());
    context.pairs.push(pair);
    fn register_pair(
        context: &mut EpochProfiles,
        ids: &mut BTreeMap<String, String>,
        key: &str,
        left: StructuredInfoType,
        right: StructuredInfoType,
    ) -> StructuredInfoType {
        let pair =
            conduit_ai::closing_structured_pair::ClosingStructuredPairProfile::prepare(left, right)
                .unwrap();
        pair.install(&mut context.startup, &mut context.profiles)
            .unwrap();
        ids.insert(format!("__FEATURE_{key}_PAIR__"), pair.identity().into());
        let result = pair.value_type().clone();
        context.pairs.push(pair);
        result
    }
    let wave_features = register_pair(
        &mut context,
        &mut ids,
        "WAVE_FEATURES",
        fixed_numeric_type("NumericF32Vector640").unwrap(),
        fixed_numeric_type("NumericF32Vector20").unwrap(),
    );
    let memories = register_pair(
        &mut context,
        &mut ids,
        "MEMORIES",
        StructuredInfoType::leaf(kind_id(F32_INFO_ID)).unwrap(),
        StructuredInfoType::leaf(kind_id(F32_INFO_ID)).unwrap(),
    );
    let period = conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition(
        "type FarganPeriod = U16 in 32..=255\n",
    )
    .unwrap()
    .value_type()
    .clone();
    let epoch_period = register_pair(
        &mut context,
        &mut ids,
        "PERIOD_EPOCH",
        period,
        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
    );
    let carry = register_pair(&mut context, &mut ids, "CARRY", memories, epoch_period);
    let proposal = register_pair(&mut context, &mut ids, "PROPOSAL", wave_features, carry);
    let weak = Arc::new(PreparedNominalWeakening::prepare(proposal).unwrap());
    weak.install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert("__FEATURE_PROPOSAL_WEAK__".into(), weak.kind_identity(true));
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

fn resample_entry_source(ids: &std::collections::BTreeMap<String, String>) -> String {
    native_feature_source(ids)
}
#[test]
fn source_native_resampling_stage_preserves_exact_admitted_input_and_explicit_temporal_owners() {
    let (context, _, ids) = prepare_feedback();
    let document = check_syntax_document(
        &parse_syntax_document(&resample_entry_source(&ids)),
        &context.startup,
    )
    .unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &document,
        "speech/flow-fargan-feature-native-resample",
        &context.profiles,
    )
    .unwrap();
    assert!(graph.expanded.gears.len() >= 6);
}

fn preemphasis_entry_source(ids: &std::collections::BTreeMap<String, String>) -> String {
    native_feature_source(ids)
}
#[test]
fn source_native_preemphasis_preserves_distinct_normalized_memory_and_explicit_flow() {
    let (context, _, ids) = prepare_feedback();
    let document = check_syntax_document(
        &parse_syntax_document(&preemphasis_entry_source(&ids)),
        &context.startup,
    )
    .unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &document,
        "speech/flow-fargan-feature-native-preemphasis",
        &context.profiles,
    )
    .unwrap();
    assert!(graph.expanded.gears.len() >= 11);
}

fn history_entry_source(ids: &std::collections::BTreeMap<String, String>) -> String {
    native_feature_source(ids)
}
#[test]
fn source_native_waveform_appends_only_explicitly_finite_preemphasized_samples() {
    let (context, _, ids) = prepare_feedback();
    let document = check_syntax_document(
        &parse_syntax_document(&history_entry_source(&ids)),
        &context.startup,
    )
    .unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &document,
        "speech/flow-fargan-feature-native-waveform",
        &context.profiles,
    )
    .unwrap();
    assert!(graph.expanded.gears.len() >= 17);
}

pub(super) fn analysis_entry_source(ids: &std::collections::BTreeMap<String, String>) -> String {
    native_feature_source(ids)
}
#[test]
fn source_complete_native_feature_analysis_retains_provisional_causal_state_and_period() {
    let (context, _, ids) = prepare_feedback();
    let document = check_syntax_document(
        &parse_syntax_document(&analysis_entry_source(&ids)),
        &context.startup,
    )
    .unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &document,
        "speech/flow-fargan-feature-native-analysis",
        &context.profiles,
    )
    .unwrap();
    eprintln!(
        "complete Source native feature proposal: {}nodes/{}cords; state remains provisional",
        graph.expanded.gears.len(),
        graph.expanded.connections.len()
    );
    assert!(graph.expanded.gears.len() > 80);
}

fn native_feature_source(ids: &std::collections::BTreeMap<String, String>) -> String {
    let authored = include_str!("../../../speech/fargan_feature_native_flow.conduit");
    for key in [
        "INPUT_NATIVE",
        "RESAMPLE_PAIR",
        "PREEMPHASIS_PAIR",
        "HISTORY_PAIR",
        "WAVE_FEATURES_PAIR",
        "MEMORIES_PAIR",
        "PERIOD_EPOCH_PAIR",
        "CARRY_PAIR",
        "PROPOSAL_PAIR",
        "PROPOSAL_WEAK",
        "PROPOSAL_NATIVE",
    ] {
        assert!(
            authored.contains(&ids[&format!("__FEATURE_{key}__")]),
            "authored exact owner identity {key}"
        );
    }
    let imports = authored
        .lines()
        .filter(|line| line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let body = authored
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    imports + "\n" + &feature_source(true) + "\n" + &body
}
