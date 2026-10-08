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
    prepare_feedback_with(
        prepared_epoch_profiles_with_capacity(true),
        conduitos::seeded_state::SeededStateOperationFactory::default(),
    )
}

pub(super) fn prepare_feedback_with(
    context: EpochProfiles,
    seeded: conduitos::seeded_state::SeededStateOperationFactory,
) -> (
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
    std::collections::BTreeMap<String, String>,
) {
    super::feature_profiles::prepare(
        context,
        super::feature_profiles::FeaturePcmGeometry::Legacy8k,
        seeded,
    )
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

pub(super) fn feedback_entry_source(ids: &std::collections::BTreeMap<String, String>) -> String {
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

pub(super) fn prepare_first_feature() -> (EpochProfiles, std::collections::BTreeMap<String, String>)
{
    let (mut context, _, mut ids) = prepare_feedback();
    let event = context
        .native
        .iter()
        .find(|profile| profile.kind_identity(true) == ids["__FEATURE_EVENT_NATIVE__"])
        .unwrap()
        .value_type()
        .clone();
    let weak = std::sync::Arc::new(PreparedNominalWeakening::prepare(event).unwrap());
    weak.install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert(
        "__FEATURE_FIRST_EVENT_WEAK__".into(),
        weak.kind_identity(true),
    );
    context.weakening.push(weak);
    (context, ids)
}

#[test]
fn first_feature_exact_profiles_preserve_pcm_period_before_source_zero_seed() {
    let (context, ids) = prepare_first_feature();
    let source = first_feature_source(&ids);
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-feature-first-analysis",
        &context.profiles,
    )
    .unwrap();
    assert!(graph.expanded.gears.len() > 80);
    eprintln!(
        "Source first native analysis zero seed: {}nodes/{}cords",
        graph.expanded.gears.len(),
        graph.expanded.connections.len()
    );
}

pub(super) fn first_feature_source(ids: &std::collections::BTreeMap<String, String>) -> String {
    let authored = include_str!("../../../speech/fargan_feature_first_flow.conduit");
    for name in [
        "__FEATURE_FIRST_EVENT_WEAK__",
        "__FEATURE_INPUT_NATIVE__",
        "__FEATURE_EVENT_NATIVE__",
    ] {
        assert!(
            authored.contains(&ids[name]),
            "exact first-feature owner {name}"
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
    imports + "\n" + &analysis_entry_source(ids) + "\n" + &body
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

pub(super) fn cycle_source(ids: &std::collections::BTreeMap<String, String>) -> String {
    let policy = include_str!("../../../speech/fargan_feature_cycle.conduit");
    for key in [
        "__FEATURE_NATIVE_PROPOSAL_WEAK__",
        "__FEATURE_PENDING_NATIVE__",
        "__FEATURE_STATE_NATIVE__",
        "__FEATURE_EVENT_NATIVE__",
        "__FEATURE_MODEL_EVENT_NATIVE__",
        "__FEATURE_ACK_WEAK__",
        "__FEATURE_PCM_NATIVE__",
        "__FEATURE_CELL__",
        "__FEATURE_ZIP__",
        "__FEATURE_INPUT_WEAK__",
        "__FEATURE_INPUT_GUARD__",
        "__FEATURE_INPUT_NATIVE__",
        "__FEATURE_ACK_MODEL_GUARD__",
        "__FEATURE_ACK_PAIR__",
        "__FEATURE_ACK_GUARD__",
    ] {
        assert!(
            policy.contains(&ids[key]),
            "retained exact Source owner {key}"
        );
    }
    let authored = [
        analysis_entry_source(ids),
        feedback_entry_source(ids),
        include_str!("../../../speech/fargan_epoch_anchor.conduit").into(),
        policy.into(),
    ]
    .join("\n");
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
    imports + "\n" + &body
}
#[test]
fn feature_cycle_authors_model_anchor_and_final_pcm_ack_before_causal_replacement() {
    let (context, _, ids) = prepare_feedback();
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let anchor=format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let source = cycle_source(&ids).replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {anchor}\n"),
    );
    let document =
        check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &document,
        "speech/flow-fargan-feature-cycle",
        &context.profiles,
    )
    .unwrap();
    assert!(graph.expanded.gears.len() > 105);
    eprintln!(
        "Source causal642/finalPCM-ACK feature cycle: {}nodes/{}cords",
        graph.expanded.gears.len(),
        graph.expanded.connections.len()
    );
}

pub(super) fn native_cycle_source(ids: &std::collections::BTreeMap<String, String>) -> String {
    let authored = [
        super::conditioning_cycle::compound_source(),
        cycle_source(ids)
            .replace("type FarganPeriod = U16 in 32..=255\n", "")
            .replace(
                include_str!("../../../speech/fargan_epoch_anchor.conduit"),
                "",
            )
            .lines()
            .filter(|line| {
                !(line.starts_with("with ") && line.ends_with(" as FarganModelFrameAnchor"))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        include_str!("../../../speech/fargan_native_cycle.conduit").into(),
    ]
    .join("\n");
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
    imports + "\n" + &body
}

#[test]
fn native_compound_cycle_authors_three_separate_feedback_domains_with_one_pcm_ack() {
    let (context, seeded, _) = prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, _) = super::conditioning_cycle::prepare_with(context, seeded);
    let (context, seeded, ids) = prepare_feedback_with(context, seeded);
    assert_eq!(seeded.offers().count(), 3);
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let selected = format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let source = native_cycle_source(&ids).replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {selected}\n"),
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-native-cycle",
        &context.profiles,
    )
    .unwrap();
    assert!(graph.expanded.gears.len() > 900);
    eprintln!("Source three-cell native cycle: {}nodes/{}cords; separate642/128/837 states, one finalPCM ACK", graph.expanded.gears.len(), graph.expanded.connections.len());
}

#[test]
fn conditioning_candidate_profile_survives_three_cell_registration() {
    let (context, seeded, signal_ids) = prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, ids) = super::conditioning_cycle::prepare_with(context, seeded);
    let identity = ids["__CONDITION_INPUT_NATIVE__"].clone();
    let before = context
        .native
        .iter()
        .find(|p| p.kind_identity(true) == identity)
        .unwrap()
        .candidate_type()
        .clone();
    let (context, _, _) = prepare_feedback_with(context, seeded);
    let after = context
        .native
        .iter()
        .find(|p| p.kind_identity(true) == identity)
        .unwrap()
        .candidate_type();
    assert_eq!(&before, after);
    let conditioning_source = include_str!("../../../speech/fargan_conditioning_cycle.conduit");
    for key in ["INPUT", "PROPOSAL", "PENDING", "EVENT", "STATE"] {
        assert!(conditioning_source.contains(&ids[&format!("__CONDITION_{key}_NATIVE__")]));
    }
    assert!(include_str!("../../../speech/fargan_signal_cycle.conduit")
        .contains(&signal_ids["__SIGNAL_STATE_PROFILE__"]));
    let source = format!("with {identity}/candidate as Candidate\nplot profile-probe (\n value: Candidate...| >> result: Candidate...|\n) = (.)\n");
    check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
}

pub(super) fn prepare_tail(
    context: EpochProfiles,
    ids: &std::collections::BTreeMap<String, String>,
) -> (
    EpochProfiles,
    String,
    StructuredInfoType,
    Vec<CapabilityOffer>,
) {
    prepare_tail_for_epochs(context, ids, 63)
}

pub(super) fn prepare_tail_for_epochs(
    mut context: EpochProfiles,
    ids: &std::collections::BTreeMap<String, String>,
    native_epochs: u32,
) -> (EpochProfiles, String, StructuredInfoType, Vec<CapabilityOffer>) {
    assert!((2..=65535).contains(&native_epochs), "finite explicit epoch profile");
    let profile = context
        .native
        .iter()
        .find(|profile| profile.kind_identity(true) == ids["__FEATURE_MODEL_EVENT_NATIVE__"])
        .unwrap()
        .clone();
    profile
        .install(&mut context.startup, &mut context.profiles, false)
        .unwrap();
    let weak = PreparedNominalWeakening::prepare(profile.value_type().clone()).unwrap();
    weak.install(&mut context.startup, &mut context.profiles, false)
        .unwrap();
    let event_contract = profile
        .contract(false)
        .unwrap()
        .checked_front()
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(port_id("result")))
        .unwrap()
        .contract
        .clone();
    conduit_semantic_catalog::install_value_repeat_capacity2_kind(
        &event_contract,
        profile.value_type(),
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    conduit_semantic_catalog::install_flow_concat_finite_kind(
        &event_contract,
        profile.value_type(),
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    let maximum = maximum_prepared_transport_value_bytes(profile.value_type()).unwrap();
    assert!(
        maximum <= 4096,
        "ordered concat profile must admit the actual feature event"
    );
    let policy = include_str!("../../../speech/fargan_utterance_tail.conduit");
    let source = format!(
        "with {}/result as FarganRawTailFeature\nwith {}/candidate as FarganTailEventCandidate\nwith {}/result as FeatureModelEvent\nwith {}/result as FarganPcm16EpochResult\n{}",
        weak.kind_identity(false),
        profile.kind_identity(false),
        profile.kind_identity(true),
        context.kinds["__PCM_VALIDATOR__"],
        policy.replace("native_epochs: U64\n", &format!("native_epochs: U64 = {native_epochs}\n"))
    );
    let mut concat = conduitos::flow_concat_finite::FlowConcatFiniteOperationFactory::default();
    let mut offers = vec![
        profile.offer(false).unwrap(),
        weak.offer(false).unwrap(),
        conduit_std_host::value_repeat::PreparedValueRepeat::capacity2(
            event_contract.clone(),
            profile.value_type().clone(),
        )
        .unwrap()
        .offer()
        .clone(),
        concat
            .install(&event_contract, profile.value_type())
            .unwrap(),
    ];
    conduit_semantic_catalog::install_flow_exactly_one_kind(
        &event_contract,
        profile.value_type(),
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    offers.push(
        conduit_std_host::flow_exactly_one::PreparedFlowExactlyOne::new(
            event_contract,
            profile.value_type().clone(),
        )
        .unwrap()
        .offer()
        .clone(),
    );
    let raw = weak.output_type().clone();
    context.weakening.push(std::sync::Arc::new(weak));
    (context, source, raw, offers)
}

pub(super) fn native_utterance_source(
    ids: &std::collections::BTreeMap<String, String>,
    tail_source: &str,
) -> String {
    let source = native_cycle_source(ids)
        + "\n"
        + &tail_source
            .lines()
            .filter(|line| !line.ends_with(" as FarganPcm16EpochResult"))
            .collect::<Vec<_>>()
            .join("\n")
        + "\n"
        + include_str!("../../../speech/fargan_native_utterance.conduit");
    let imports: std::collections::BTreeSet<_> = source
        .lines()
        .filter(|line| line.starts_with("with "))
        .collect();
    imports.into_iter().collect::<Vec<_>>().join("\n")
        + "\n"
        + &source
            .lines()
            .filter(|line| !line.starts_with("with "))
            .collect::<Vec<_>>()
            .join("\n")
}

#[test]
fn native_utterance_authors_late_singleton_and_two_tails_inside_three_feedback_domains() {
    let (context, seeded, _) = prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, _) = super::conditioning_cycle::prepare_with(context, seeded);
    let (context, seeded, ids) = prepare_feedback_with(context, seeded);
    let (context, tail, _, _) = prepare_tail(context, &ids);
    assert_eq!(seeded.offers().count(), 3);
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let anchor = format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let source = native_utterance_source(&ids, &tail)
        .replace(
            "selected: FarganModelFrameAnchor\n",
            &format!("selected: FarganModelFrameAnchor = {anchor}\n"),
        )
        .replace("native_epochs: U64\n", "native_epochs: U64 = 63\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-native-utterance",
        &context.profiles,
    )
    .unwrap();
    eprintln!(
        "Source finite native utterance: {}nodes/{}cords",
        graph.expanded.gears.len(),
        graph.expanded.connections.len()
    );
}

#[test]
fn utterance_tail_candidates_keep_feature_shape_and_assign_source_epochs() {
    let (context, _, ids) = prepare_feedback();
    let (context, source, _, _) = prepare_tail(context, &ids);
    let document =
        check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    for name in [
        "speech/fargan-first-tail-candidate",
        "speech/fargan-second-tail-candidate",
        "speech/fargan-utterance-tail",
        "speech/flow-fargan-last-native-feature",
        "speech/flow-fargan-native-pcm-ack",
    ] {
        expand_canonical_plot_for_authoring(&document, name, &context.profiles).unwrap();
    }
    assert!(source.contains("epoch: native_epochs}"));
    assert!(source.contains("epoch: (native_epochs + 1)"));
    eprintln!("Source tail event fits ordered concat limit4096B");
    eprintln!(
        "retained tail input alias: {}",
        source.lines().next().unwrap()
    );
}
