//! Mechanical Native/pair/feedback preparation; Source owns acoustic policy.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FeaturePcmGeometry {
    Legacy8k,
    Direct16k,
}
impl FeaturePcmGeometry {
    fn input_type(self) -> &'static str {
        match self {
            Self::Legacy8k => "FarganFeatureInputEpoch",
            Self::Direct16k => "FarganFeatureInputEpoch16k",
        }
    }
    fn event_type(self) -> &'static str {
        match self {
            Self::Legacy8k => "FarganFeaturePcmEpoch",
            Self::Direct16k => "FarganFeaturePcmEpoch16k",
        }
    }
}
pub(super) fn prepare(
    mut context: EpochProfiles,
    geometry: FeaturePcmGeometry,
    mut seeded: conduitos::seeded_state::SeededStateOperationFactory,
) -> (
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
    std::collections::BTreeMap<String, String>,
) {
    use std::{collections::BTreeMap, sync::Arc};
    let definition = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../speech/fargan_epoch_feedback.conduit")
        + "\n"
        + include_str!("../../../speech/fargan_feature_epoch_contracts.conduit");
    let direct_definition = definition.clone()
        + "\n"
        + include_str!("../../../speech/fargan_feature_direct16k_contracts.conduit");
    let mut ids = BTreeMap::new();
    let mut native = BTreeMap::new();
    for (key, name) in [
        ("INPUT", geometry.input_type()),
        ("PROPOSAL", "FarganFeatureProposalEpoch"),
        ("PENDING", "FarganFeaturePendingState"),
        ("STATE", "FarganFeatureEpochFeedback"),
        ("EVENT", geometry.event_type()),
    ] {
        let selected_definition =
            if geometry == FeaturePcmGeometry::Direct16k && matches!(key, "INPUT" | "EVENT") {
                &direct_definition
            } else {
                &definition
            };
        let profile =
            Arc::new(PreparedNativeProfile::check_definition(selected_definition, name).unwrap());
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
    let event_definition = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../speech/fargan_conditioning_epoch_contracts.conduit");
    let model_event = Arc::new(
        PreparedNativeProfile::check_definition(&event_definition, "FarganFeatureConditionEpoch")
            .unwrap(),
    );
    model_event
        .install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert(
        "__FEATURE_MODEL_EVENT_NATIVE__".into(),
        model_event.kind_identity(true),
    );
    context.native.push(model_event);
    let raw_proposal =
        Arc::new(PreparedNominalWeakening::prepare(native["PROPOSAL"].clone()).unwrap());
    raw_proposal
        .install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert(
        "__FEATURE_NATIVE_PROPOSAL_WEAK__".into(),
        raw_proposal.kind_identity(true),
    );
    context.weakening.push(raw_proposal);
    let ack = register_pair(
        &mut context,
        &mut ids,
        "ACK",
        native["PENDING"].clone(),
        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
    );
    let weak_ack = Arc::new(PreparedNominalWeakening::prepare(ack).unwrap());
    weak_ack
        .install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert("__FEATURE_ACK_WEAK__".into(), weak_ack.kind_identity(true));
    let ack_guard =
        conduit_ai::fixed_numeric_guard::FixedGuardProfile::prepare(weak_ack.output_type().clone())
            .unwrap();
    ack_guard
        .install(&mut context.startup, &mut context.profiles)
        .unwrap();
    ids.insert(
        "__FEATURE_ACK_GUARD__".into(),
        ack_guard.contract().unwrap().kind_id.as_str().into(),
    );
    context.guards.push(ack_guard);
    context.weakening.push(weak_ack);
    let pcm = context
        .native
        .iter()
        .find(|profile| profile.kind_identity(true) == context.kinds["__PCM_VALIDATOR__"])
        .unwrap()
        .value_type()
        .clone();
    let ack_model_guard = conduit_ai::fixed_numeric_guard::FixedGuardProfile::prepare(pcm).unwrap();
    let existing_ack_guard = context
        .guards
        .iter()
        .any(|profile| profile.contract().unwrap() == ack_model_guard.contract().unwrap());
    if !existing_ack_guard {
        ack_model_guard
            .install(&mut context.startup, &mut context.profiles)
            .unwrap();
    }
    ids.insert(
        "__FEATURE_ACK_MODEL_GUARD__".into(),
        ack_model_guard.contract().unwrap().kind_id.as_str().into(),
    );
    ids.insert(
        "__FEATURE_PCM_NATIVE__".into(),
        context.kinds["__PCM_VALIDATOR__"].clone(),
    );
    if !existing_ack_guard {
        context.guards.push(ack_model_guard);
    }
    (context, seeded, ids)
}
