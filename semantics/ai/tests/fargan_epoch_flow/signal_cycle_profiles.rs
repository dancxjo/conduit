use super::*;

pub(super) fn prepared_signal_cycle_profiles() -> (
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
    std::collections::BTreeMap<String, String>,
) {
    prepared_signal_cycle_profiles_with_capacity(false)
}
pub(super) fn prepared_signal_cycle_profiles_with_capacity(
    capacity64: bool,
) -> (
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
    std::collections::BTreeMap<String, String>,
) {
    let mut context = prepared_epoch_profiles_with_capacity(capacity64);
    let definition = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../speech/fargan_epoch_feedback.conduit");
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
    let mut ids = std::collections::BTreeMap::new();
    let mut seeded = conduitos::seeded_state::SeededStateOperationFactory::default();
    let kind = conduit_semantic_catalog::install_seeded_state_flow_specialized_kind(
        &shape_contract(&state),
        &state,
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    let offer = seeded
        .install_flow_specialized_frame16k(&shape_contract(&state), &state)
        .unwrap();
    assert_eq!(offer.kind_id, kind);
    ids.insert("__SIGNAL_CELL__".into(), kind.as_str().to_owned());
    let pair_kind = conduit_semantic_catalog::install_flow_zip_feedback_specialized_kind(
        &shape_contract(&state),
        &state,
        &shape_contract(&event),
        &event,
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    let offer = context
        .zip
        .install_feedback_specialized(
            &shape_contract(&state),
            &state,
            &shape_contract(&event),
            &event,
        )
        .unwrap();
    assert_eq!(offer.kind_id, pair_kind);
    ids.insert("__SIGNAL_ZIP__".into(), pair_kind.as_str().to_owned());
    let pair = PreparedTypedTuplePairEncoder::new(
        state.clone(),
        maximum_prepared_transport_value_bytes(&state).unwrap(),
        event.clone(),
        maximum_prepared_transport_value_bytes(&event).unwrap(),
    )
    .unwrap();
    let weak =
        std::sync::Arc::new(PreparedNominalWeakening::prepare(pair.value_type().clone()).unwrap());
    weak.install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert("__SIGNAL_PAIR_WEAK__".into(), weak.kind_identity(true));
    let guard =
        conduit_ai::fixed_numeric_guard::FixedGuardProfile::prepare(weak.output_type().clone())
            .unwrap();
    guard
        .install(&mut context.startup, &mut context.profiles)
        .unwrap();
    ids.insert(
        "__SIGNAL_GUARD__".into(),
        guard.contract().unwrap().kind_id.as_str().to_owned(),
    );
    context.guards.push(guard);
    context.weakening.push(weak);
    let nextweak = std::sync::Arc::new(
        PreparedNominalWeakening::prepare(
            context
                .native
                .iter()
                .find(|p| p.kind_identity(true) == context.kinds["__PCM_VALIDATOR__"])
                .unwrap()
                .value_type()
                .clone(),
        )
        .unwrap(),
    );
    nextweak
        .install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert("__SIGNAL_NEXT_WEAK__".into(), nextweak.kind_identity(true));
    context.weakening.push(nextweak);
    let stateprofile = std::sync::Arc::new(
        PreparedNativeProfile::check_definition(&definition, "FarganSignalEpochFeedback").unwrap(),
    );
    stateprofile
        .install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert(
        "__SIGNAL_STATE_PROFILE__".into(),
        stateprofile.kind_identity(true),
    );
    ids.insert(
        "__SIGNAL_EPOCH_PROFILE__".into(),
        context.kinds["__EPOCH_INPUT_VALIDATOR__"].clone(),
    );
    context.native.push(stateprofile);
    (context, seeded, ids)
}
