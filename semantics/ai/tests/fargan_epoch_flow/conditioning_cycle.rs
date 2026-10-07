//! Exact generic owners for the separate conditioning-history feedback domain.
use super::*;
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn prepare() -> (
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
    BTreeMap<String, String>,
) {
    prepare_with(
        prepared_epoch_profiles_with_capacity(true),
        conduitos::seeded_state::SeededStateOperationFactory::default(),
    )
}
pub(super) fn prepare_with(
    mut context: EpochProfiles,
    mut seeded: conduitos::seeded_state::SeededStateOperationFactory,
) -> (
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
    BTreeMap<String, String>,
) {
    let definition = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../speech/fargan_epoch_feedback.conduit")
        + "\n"
        + include_str!("../../../speech/fargan_conditioning_epoch_contracts.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(&definition), &StartupCatalog::new()).unwrap();
    let ty = |name: &str| {
        checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    let mut ids = BTreeMap::new();
    for (key, name) in [
        ("INPUT", "FarganConditioningInputEpoch"),
        ("PROPOSAL", "FarganConditioningProposalEpoch"),
        ("PENDING", "FarganConditioningPendingHistory"),
        ("STATE", "FarganConditioningEpochFeedback"),
        ("EVENT", "FarganSignalConditionEpoch"),
    ] {
        let profile = Arc::new(PreparedNativeProfile::check_definition(&definition, name).unwrap());
        profile
            .install(&mut context.startup, &mut context.profiles, true)
            .unwrap();
        ids.insert(
            format!("__CONDITION_{key}_NATIVE__"),
            profile.kind_identity(true),
        );
        context.native.push(profile);
    }
    let state = ty("FarganConditioningEpochFeedback");
    let event = ty("FarganFeatureConditionEpoch");
    let cell = conduit_semantic_catalog::install_seeded_state_flow_specialized_kind(
        &shape_contract(&state),
        &state,
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    assert_eq!(
        seeded
            .install_flow_specialized_frame16k(&shape_contract(&state), &state)
            .unwrap()
            .kind_id,
        cell
    );
    ids.insert("__CONDITION_CELL__".into(), cell.as_str().into());
    let zip = conduit_semantic_catalog::install_flow_zip_feedback_specialized_kind(
        &shape_contract(&state),
        &state,
        &shape_contract(&event),
        &event,
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    assert_eq!(
        context
            .zip
            .install_feedback_specialized(
                &shape_contract(&state),
                &state,
                &shape_contract(&event),
                &event
            )
            .unwrap()
            .kind_id,
        zip
    );
    ids.insert("__CONDITION_ZIP__".into(), zip.as_str().into());
    let input_pair = PreparedTypedTuplePairEncoder::new(
        state.clone(),
        maximum_prepared_transport_value_bytes(&state).unwrap(),
        event.clone(),
        maximum_prepared_transport_value_bytes(&event).unwrap(),
    )
    .unwrap()
    .value_type()
    .clone();
    fn weak(
        context: &mut EpochProfiles,
        ids: &mut BTreeMap<String, String>,
        key: &str,
        value: StructuredInfoType,
    ) -> StructuredInfoType {
        let weak = Arc::new(PreparedNominalWeakening::prepare(value).unwrap());
        weak.install(&mut context.startup, &mut context.profiles, true)
            .unwrap();
        ids.insert(
            format!("__CONDITION_{key}_WEAK__"),
            weak.kind_identity(true),
        );
        let result = weak.output_type().clone();
        context.weakening.push(weak);
        result
    }
    fn pair(
        context: &mut EpochProfiles,
        ids: &mut BTreeMap<String, String>,
        key: &str,
        left: StructuredInfoType,
        right: StructuredInfoType,
    ) -> StructuredInfoType {
        let result = PreparedTypedTuplePairEncoder::new(
            left.clone(),
            maximum_prepared_transport_value_bytes(&left).unwrap(),
            right.clone(),
            maximum_prepared_transport_value_bytes(&right).unwrap(),
        )
        .unwrap()
        .value_type()
        .clone();
        let pair =
            conduit_ai::closing_structured_pair::ClosingStructuredPairProfile::prepare(left, right)
                .unwrap();
        pair.install(&mut context.startup, &mut context.profiles)
            .unwrap();
        ids.insert(format!("__CONDITION_{key}_PAIR__"), pair.identity().into());
        context.pairs.push(pair);
        result
    }
    fn guard(
        context: &mut EpochProfiles,
        ids: &mut BTreeMap<String, String>,
        key: &str,
        value: StructuredInfoType,
    ) {
        let guard = conduit_ai::fixed_numeric_guard::FixedGuardProfile::prepare(value).unwrap();
        guard
            .install(&mut context.startup, &mut context.profiles)
            .unwrap();
        ids.insert(
            format!("__CONDITION_{key}_GUARD__"),
            guard.contract().unwrap().kind_id.as_str().into(),
        );
        context.guards.push(guard);
    }
    let input_raw = weak(&mut context, &mut ids, "INPUT_PAIR", input_pair);
    guard(&mut context, &mut ids, "INPUT", input_raw);
    let carry_raw = weak(
        &mut context,
        &mut ids,
        "INPUT",
        ty("FarganConditioningInputEpoch"),
    );
    let numeric = pair(
        &mut context,
        &mut ids,
        "NUMERIC",
        fixed_numeric_type("NumericF32Vector320").unwrap(),
        fixed_numeric_type("NumericHistory2x64").unwrap(),
    );
    let proposal = pair(&mut context, &mut ids, "PROPOSAL", numeric, carry_raw);
    weak(&mut context, &mut ids, "PROPOSAL_PAIR", proposal);
    weak(
        &mut context,
        &mut ids,
        "PROPOSAL",
        ty("FarganConditioningProposalEpoch"),
    );
    guard(
        &mut context,
        &mut ids,
        "ACK_INPUT",
        ty("FarganPcm16EpochResult"),
    );
    let ack = pair(
        &mut context,
        &mut ids,
        "ACK",
        ty("FarganConditioningPendingHistory"),
        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
    );
    let ack_raw = weak(&mut context, &mut ids, "ACK", ack);
    guard(&mut context, &mut ids, "ACK", ack_raw);
    (context, seeded, ids)
}

#[test]
fn conditioning_history_feedback_owners_have_exact_separate_bounded_domains() {
    let (_, _, ids) = prepare();
    for (name, id) in ids {
        eprintln!("{name}={id}");
    }
}

#[test]
fn authored_conditioning_cycle_checks_final_pcm_ack_and_expands_without_replanning() {
    let (context, _, ids) = prepare();
    let cycle = String::from(include_str!(
        "../../../speech/fargan_conditioning_cycle.conduit"
    ));
    for value in ids.values() {
        assert!(cycle.contains(value), "authored owner identity {value}");
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
    let source = imports
        + "\n"
        + &declarations::exact_epoch_declarations()
            .lines()
            .filter(|line| !line.starts_with("type Numeric"))
            .collect::<Vec<_>>()
            .join("\n")
        + "\n"
        + include_str!("../../../speech/fargan_epoch_feedback.conduit")
        + "\n"
        + include_str!("../../../speech/fargan_conditioning_epoch_contracts.conduit")
        + "\n"
        + include_str!("../../../speech/fargan_conditioning_flow.conduit")
        + "\n"
        + &body
        + "\n"
        + include_str!("../../../speech/fargan_epoch_anchor.conduit");
    let source = source.replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {selected}\n"),
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    for (name, cases) in [
        (
            "speech/flow-fargan-conditioning-input-matches",
            vec![(7, 7, true), (7, 8, false), (u64::MAX, u64::MAX, false)],
        ),
        (
            "speech/flow-fargan-conditioning-ack-matches",
            vec![(0, 0, true), (7, 7, true), (u64::MAX, u64::MAX, false)],
        ),
    ] {
        let matcher =
            expand_canonical_plot_for_authoring(&checked, name, &context.profiles).unwrap();
        let ConfigurationValue::Text(encoded) = &matcher.expanded.gears[0].configuration[0].value
        else {
            panic!("matcher program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        for (next, event, accepted) in cases {
            let input = epoch_pair_fixture(&program.input_type, "", next, event)
                .canonical_bytes()
                .unwrap();
            let expected = InfoBool::new(accepted).encode();
            assert_eq!(program.evaluate(&input).unwrap(), expected);
            assert_eq!(prepared.evaluate(&input).unwrap(), expected);
        }
    }
    let anchor = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-model-anchor-matches",
        &context.profiles,
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &anchor.expanded.gears[0].configuration[0].value else {
        panic!("anchor program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for target in [
        "",
        "artifact_identity",
        "model_descriptor_identity",
        "session_basis_identity",
        "precision",
    ] {
        let input = anchor_fixture(&program.input_type, target, false)
            .canonical_bytes()
            .unwrap();
        let expected = InfoBool::new(target.is_empty()).encode();
        assert_eq!(program.evaluate(&input).unwrap(), expected);
        assert_eq!(prepared.evaluate(&input).unwrap(), expected);
    }
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-conditioning-cycle",
        &context.profiles,
    )
    .unwrap();
    eprintln!(
        "Source conditioning feedback: {}nodes/{}cords",
        expanded.expanded.gears.len(),
        expanded.expanded.connections.len()
    );
}

pub(super) fn compound_source() -> String {
    let condition = include_str!("../../../speech/fargan_conditioning_cycle.conduit");
    let signal = include_str!("../../../speech/fargan_signal_cycle.conduit");
    let imports = condition
        .lines()
        .chain(signal.lines())
        .filter(|line| line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let condition_body = condition
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let signal_body = signal
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    [
        imports,
        unbound_epoch_source(),
        String::from(include_str!(
            "../../../speech/fargan_epoch_feedback.conduit"
        )),
        String::from(include_str!(
            "../../../speech/fargan_conditioning_epoch_contracts.conduit"
        )),
        String::from(include_str!(
            "../../../speech/fargan_conditioning_flow.conduit"
        )),
        condition_body,
        signal_body,
        String::from(include_str!(
            "../../../speech/fargan_compound_cycle.conduit"
        )),
    ]
    .join("\n")
}

#[test]
fn authored_compound_cycle_keeps_two_cells_correlated_by_one_final_pcm_result() {
    let (context, seeded, signal_ids) = prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, condition_ids) = prepare_with(context, seeded);
    assert_eq!(seeded.offers().count(), 2);
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let selected=format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let source = compound_source().replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {selected}\n"),
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-compound-cycle",
        &context.profiles,
    )
    .unwrap();
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == signal_ids["__SIGNAL_CELL__"])
            .count(),
        1
    );
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == condition_ids["__CONDITION_CELL__"])
            .count(),
        1
    );
    eprintln!("Source compoundcycle: {}nodes/{}cords; separately admitted seeds, no session/runtime admission claim",expanded.expanded.gears.len(),expanded.expanded.connections.len());
}

fn anchor_fixture(ty: &StructuredInfoType, target: &str, alter: bool) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), anchor_fixture(representation, target, alter))
                .unwrap()
        }
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        anchor_fixture(field.value_type(), target, field.name() == target),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Collection { element, length } => StructuredInfoValue::collection(
            ty.clone(),
            (0..length)
                .map(|_| anchor_fixture(element, target, alter))
                .collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/u8" => {
            StructuredInfoValue::leaf(ty.clone(), vec![if alter { 2 } else { 1 }]).unwrap()
        }
        StructuredInfoTypeShape::Variant { cases, .. } if alter => {
            let case = cases
                .iter()
                .find(|case| case.tag() == "compact_signed_q7")
                .unwrap();
            StructuredInfoValue::variant(
                ty.clone(),
                case.tag(),
                declarations::fixture_value(case.payload_type()),
            )
            .unwrap()
        }
        _ => declarations::fixture_value(ty),
    }
}
