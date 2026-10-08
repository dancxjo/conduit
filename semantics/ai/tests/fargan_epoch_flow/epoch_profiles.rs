use super::*;

pub(super) fn catalogs(capacity64: bool) -> (StartupCatalog, ProfileCatalog) {
    let mut s = StartupCatalog::new();
    let mut p = ProfileCatalog::new();
    if capacity64 {
        install_fixed_numeric_catalogs_capacity64(&mut s, &mut p).unwrap();
    } else {
        install_fixed_numeric_catalogs(&mut s, &mut p).unwrap();
    }
    install_fixed_numeric_pair_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_flow::install_affine_flow_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_linear_flow::install_linear_flow_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_embedding_flow::install_embedding_flow_catalogs(&mut s, &mut p)
        .unwrap();
    if capacity64 {
        conduit_ai::fixed_numeric_temporal::install_closing_numeric_catalogs_capacity64(
            &mut s, &mut p,
        )
        .unwrap();
    } else {
        conduit_ai::fixed_numeric_temporal::install_closing_numeric_catalogs(&mut s, &mut p)
            .unwrap();
    }
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
pub(super) fn shape_contract(ty: &StructuredInfoType) -> CheckedValueContract {
    CheckedValueContract::new(
        ty.profile().unwrap().value_kind().clone(),
        maximum_prepared_transport_value_bytes(ty).unwrap(),
        vec![],
    )
    .unwrap()
}
pub(super) struct EpochProfiles {
    pub(super) startup: StartupCatalog,
    pub(super) profiles: ProfileCatalog,
    pub(super) imports: String,
    pub(super) kinds: std::collections::BTreeMap<String, String>,
    pub(super) native: Vec<std::sync::Arc<PreparedNativeProfile>>,
    pub(super) weakening: Vec<std::sync::Arc<PreparedNominalWeakening>>,
    pub(super) guards: Vec<conduit_ai::fixed_numeric_guard::FixedGuardProfile>,
    pub(super) zip: conduitos::flow_zip::FlowZipOperationFactory,
    pub(super) pairs: Vec<conduit_ai::closing_structured_pair::ClosingStructuredPairProfile>,
}
pub(super) fn prepared_epoch_profiles() -> EpochProfiles {
    prepared_epoch_profiles_with_capacity(false)
}
pub(super) fn prepared_epoch_profiles_with_capacity(capacity64: bool) -> EpochProfiles {
    let (mut startup, mut profiles) = catalogs(capacity64);
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
    let mut native = vec![];
    let mut weakening = vec![];
    let zip = conduitos::flow_zip::FlowZipOperationFactory::frame16k();
    let mut pairs = vec![];
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
        let profile = std::sync::Arc::new(
            PreparedNativeProfile::check_definition(&definition, name).unwrap(),
        );
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
        native.push(profile);
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
    weakening.push(std::sync::Arc::new(entry));
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
        let selected = conduit_ai::closing_structured_pair::ClosingStructuredPairProfile::prepare(
            left.clone(),
            right.clone(),
        )
        .unwrap();
        selected.install(&mut startup, &mut profiles).unwrap();
        kinds.insert(format!("__{key}_PAIR__"), selected.identity().to_owned());
        pairs.push(selected);
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
        weakening.push(std::sync::Arc::new(weak));
    }
    EpochProfiles {
        startup,
        profiles,
        imports,
        kinds,
        native,
        weakening,
        guards: vec![guard],
        zip,
        pairs,
    }
}
