//! Optional Source diagnostic registration; it grants no feedback authority.
use super::*;
use std::sync::Arc;

pub(super) fn prepare(mut context: EpochProfiles) -> (EpochProfiles, String) {
    let definition = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../speech/fargan_epoch_trace.conduit");
    let profile = Arc::new(
        PreparedNativeProfile::check_definition(&definition, "FarganTraceProposal").unwrap(),
    );
    profile
        .install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    let identity = profile.kind_identity(true);
    context.native.push(profile);
    (context, identity)
}

pub(super) fn source(identity: &str) -> String {
    include_str!("../../../speech/fargan_epoch_trace.conduit").to_owned()
        + "\n"
        + include_str!("../../../speech/fargan_trace_flow.conduit")
        + "\n"
        + &include_str!("../../../speech/fargan_conditioning_trace_cycle.conduit")
            .replace("__FARGAN_TRACE_PROFILE__", identity)
        + "\n"
        + include_str!("../../../speech/fargan_compound_cycle_traced.conduit")
}

pub(super) fn check_source() {
    let (context, seeded, _) = prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, _) = conditioning_cycle::prepare_with(context, seeded);
    let (context, identity) = prepare(context);
    assert_eq!(seeded.offers().count(), 2);
    let base = conditioning_cycle::compound_source();
    let source = base + "\n" + &source(&identity);
    let imports = source
        .lines()
        .filter(|line| line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let body = source
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let source = imports + "\n" + &body;
    let source = source.replace("selected: FarganModelFrameAnchor\n", &format!("selected: FarganModelFrameAnchor = {}\n",{
        let receipt = format!("[{}]",vec!["1";32].join(","));
        format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}")
    }));
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-conditioning-cycle-traced",
        &context.profiles,
    )
    .unwrap();
    assert!(expanded.expanded.gears.len() > 20);
    let compound = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-compound-cycle-traced",
        &context.profiles,
    )
    .unwrap();
    assert!(compound.expanded.gears.len() > 700);
    eprintln!(
        "traced conditioning:{}nodes/{}cords; traced compound:{}nodes/{}cords",
        expanded.expanded.gears.len(),
        expanded.expanded.connections.len(),
        compound.expanded.gears.len(),
        compound.expanded.connections.len()
    );
}

#[test]
fn traced_conditioning_source_expands_exact_separate_observation_outputs() {
    check_source();
}

#[cfg(test)]
pub(super) fn native_utterance_source(
    ids: &std::collections::BTreeMap<String, String>,
    tail: &str,
    identity: &str,
) -> String {
    let source = feature_cycle::native_utterance_source(ids, tail)
        + "\n"
        + &source(identity)
        + "\n"
        + include_str!("../../../speech/fargan_native_cycle_traced.conduit")
        + "\n"
        + include_str!("../../../speech/fargan_native_utterance_traced.conduit");
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
fn traced_native_utterance_source_keeps_all_three_cells_and_separate_bounded_outputs() {
    let (context, seeded, _) = prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, _) = conditioning_cycle::prepare_with(context, seeded);
    let (context, seeded, ids) = feature_cycle::prepare_feedback_with(context, seeded);
    let (context, tail, _, _) = feature_cycle::prepare_tail(context, &ids);
    let (context, identity) = prepare(context);
    assert_eq!(seeded.offers().count(), 3);
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let anchor = format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let source = native_utterance_source(&ids, &tail, &identity)
        .replace(
            "selected: FarganModelFrameAnchor\n",
            &format!("selected: FarganModelFrameAnchor = {anchor}\n"),
        )
        .replace("native_epochs: U64\n", "native_epochs: U64 = 63\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-native-utterance-traced",
        &context.profiles,
    )
    .unwrap();
    assert!(expanded.expanded.gears.len() < 1024);
    assert!(expanded.expanded.connections.len() < 2048);
    for name in [
        "FarganFeatureConditionTrace",
        "FarganConditionHistoryTrace",
        "FarganCommittedResultTrace",
    ] {
        let ty = &checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type;
        assert!(maximum_prepared_transport_value_bytes(ty).unwrap() <= 16384);
    }
}
