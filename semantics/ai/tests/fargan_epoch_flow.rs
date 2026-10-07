#![cfg(feature = "kernel-operation-owners")]
#[path = "fargan_epoch_contracts.rs"]
mod declarations;
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_pair_catalog::*, native_profile::PreparedNativeProfile,
    nominal_weakening::PreparedNominalWeakening,
};
use conduit_core::*;
use conduit_plot::*;
fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut s = StartupCatalog::new();
    let mut p = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut s, &mut p).unwrap();
    install_fixed_numeric_pair_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_flow::install_affine_flow_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_linear_flow::install_linear_flow_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_embedding_flow::install_embedding_flow_catalogs(&mut s, &mut p)
        .unwrap();
    conduit_ai::fixed_numeric_temporal::install_closing_numeric_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_pair_flow::install_fixed_flow_pair_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_dsp_catalog::install_fixed_dsp_flow_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_integer_narrowing::install_checked_integer_flow_catalogs(
        &mut s, &mut p,
    )
    .unwrap();
    conduit_ai::fixed_numeric_float_integer::install_float_integer_catalogs(&mut s, &mut p)
        .unwrap();
    conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition(
        "type FarganPeriod = U16 in 32..=255\n",
    )
    .unwrap()
    .install(&mut s, &mut p, true)
    .unwrap();
    (s, p)
}
fn shape_contract(ty: &StructuredInfoType) -> CheckedValueContract {
    CheckedValueContract::new(
        ty.profile().unwrap().value_kind().clone(),
        maximum_prepared_transport_value_bytes(ty).unwrap(),
        vec![],
    )
    .unwrap()
}
fn prepared_epoch_profiles() -> (
    StartupCatalog,
    ProfileCatalog,
    String,
    std::collections::BTreeMap<String, String>,
) {
    let (mut startup, mut profiles) = catalogs();
    let definition = declarations::exact_epoch_declarations();
    let types =
        check_syntax_document(&parse_syntax_document(&definition), &StartupCatalog::new()).unwrap();
    let ty = |name: &str| {
        types
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    let mut imports = String::new();
    let mut kinds = std::collections::BTreeMap::new();
    for (key, name) in [
        ("EPOCH_INPUT", "FarganFloatEpochInput"),
        ("PHASE1", "FarganFloatPhase1"),
        ("PHASE2", "FarganFloatPhase2"),
        ("PHASE3", "FarganFloatPhase3"),
        ("PHASE4", "FarganFloatEpochProposal"),
        ("PCM", "FarganPcm16EpochResult"),
    ] {
        let profile = PreparedNativeProfile::check_definition(&definition, name).unwrap();
        profile.install(&mut startup, &mut profiles, true).unwrap();
        let id = profile.kind_identity(true);
        kinds.insert(format!("__{key}_VALIDATOR__"), id.clone());
        if key.starts_with("PHASE") || key == "PCM" {
            let alias = if key == "PCM" {
                "FarganPcm16Candidate".to_owned()
            } else {
                format!("FarganPhase{}Candidate", &key[5..])
            };
            imports.push_str(&format!("with {id}/candidate as {alias}\n"));
        }
    }
    let guard =
        conduit_ai::fixed_numeric_guard::FixedGuardProfile::prepare(ty("FarganFloatEpochInput"))
            .unwrap();
    guard.install(&mut startup, &mut profiles).unwrap();
    kinds.insert(
        "__EPOCH_GUARD__".into(),
        guard.contract().unwrap().kind_id.as_str().to_owned(),
    );
    let entry = PreparedNominalWeakening::prepare(ty("FarganFloatEpochInput")).unwrap();
    entry.install(&mut startup, &mut profiles, true).unwrap();
    kinds.insert(
        "__EPOCH_INPUT_WEAKENING__".into(),
        entry.kind_identity(true),
    );
    for (key, left, right, alias) in [
        (
            "PHASE",
            ty("FarganEpochPhaseCarry"),
            ty("FarganSubframeResult"),
            "FarganEpochPhasePair",
        ),
        (
            "PCM",
            ty("FarganEpochFinalCarry"),
            fixed_numeric_type("NumericI16Vector160").unwrap(),
            "FarganPcm16Pair",
        ),
    ] {
        let id = conduit_semantic_catalog::install_flow_zip_finite_specialized_kind(
            &shape_contract(&left),
            &left,
            &shape_contract(&right),
            &right,
            &mut startup,
            &mut profiles,
        )
        .unwrap();
        kinds.insert(format!("__{key}_ZIP__"), id.as_str().to_owned());
        let pair = PreparedTypedTuplePairEncoder::new(
            left.clone(),
            maximum_prepared_transport_value_bytes(&left).unwrap(),
            right.clone(),
            maximum_prepared_transport_value_bytes(&right).unwrap(),
        )
        .unwrap();
        let weak = PreparedNominalWeakening::prepare(pair.value_type().clone()).unwrap();
        weak.install(&mut startup, &mut profiles, true).unwrap();
        kinds.insert(format!("__{key}_WEAKENING__"), weak.kind_identity(true));
        imports.push_str(&format!(
            "with {}/result as {alias}\n",
            weak.kind_identity(true)
        ));
    }
    (startup, profiles, imports, kinds)
}
#[test]
fn authored_epoch_imports_and_owners_match_exact_checked_profiles() {
    let (_, _, imports, kinds) = prepared_epoch_profiles();
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
    let (startup, profiles, _, _) = prepared_epoch_profiles();
    let mut source = format!(
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
    );
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let selected=format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    source = source.replace(
        "selected: FarganModelFrameAnchor",
        &format!("selected: FarganModelFrameAnchor = {selected}"),
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "speech/flow-fargan-float-epoch", &profiles)
            .unwrap();
    eprintln!(
        "complete epoch: {} nodes, {} cords",
        expanded.expanded.gears.len(),
        expanded.expanded.connections.len()
    );
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
