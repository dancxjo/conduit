//! Private transfer keeps complete provenance and normal canonical refusal.
use conduit_birth_plot::{BirthDraft, BirthFaceBasis, BirthPlotChoice};
use conduit_body::ResidentPlot;
use conduit_face::{FaceInteraction, FaceInteractionArgument};
use conduitos_creche_core::data::{self, InteractionRequest};
#[path = "interaction/mask.rs"]
mod mask;
fn fixture() -> (BirthDraft, InteractionRequest) {
    let draft = BirthDraft::new(
        "550e8400-e29b-41d4-a716-446655440000".into(),
        (0..4)
            .map(|index| BirthPlotChoice {
                title: format!("Plot {index}"),
                search_text: format!("plot {index}"),
                plot: ResidentPlot::new(
                    format!("source/{index}").into(),
                    format!("checked/{index}").into(),
                ),
                refusal: None,
                selected: true,
            })
            .collect(),
    )
    .unwrap();
    let basis = BirthFaceBasis {
        host_id: "host/fixture".into(),
        boot_id: "boot/fixture".into(),
        encounter_id: "fixture/interaction".into(),
        producer_plot: conduit_core::PlotIdentity {
            source_document_id: "source/producer".into(),
            checked_plot_id: "checked/producer".into(),
            expanded_plot_id: "expanded/producer".into(),
        },
        producer_plan_id: "plan/producer".into(),
    };
    let face = draft.face(&basis).unwrap();
    let show = mask::acknowledged_fixture_show(&face);
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == "creche.plot.0")
        .unwrap();
    let input = FaceInteraction::new(
        &face,
        &show,
        &action.identity,
        &action.target,
        vec![FaceInteractionArgument {
            name: "value".into(),
            value_kind: "value/bool".into(),
            value: vec![0],
        }],
        1,
    )
    .unwrap();
    (
        draft,
        InteractionRequest {
            revision: draft_revision(&face),
            basis,
            show,
            input,
        },
    )
}
fn draft_revision(face: &conduit_face::Face) -> u32 {
    face.revision.try_into().unwrap()
}
#[test]
fn complete_binary_roundtrip_toggles_and_restores_four_plot_draft() {
    let (mut draft, input) = fixture();
    let encoded = data::encode_interaction(&input).unwrap();
    assert!(encoded.len() <= conduitos_creche_core::wire::MAX_BLOB_BYTES as usize);
    let decoded = data::decode_interaction(&encoded).unwrap();
    assert_eq!(decoded.show, input.show);
    assert_eq!(decoded.input, input.input);
    assert_eq!(decoded.basis, input.basis);
    draft
        .apply_face_interaction(&decoded.basis, &decoded.show, &decoded.input)
        .unwrap();
    assert!(!draft.choices()[0].selected);
    assert_eq!(
        draft
            .choices()
            .iter()
            .filter(|choice| choice.selected)
            .count(),
        3
    );
    let face = draft.face(&decoded.basis).unwrap();
    let show = mask::acknowledged_fixture_show(&face);
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == "creche.plot.0")
        .unwrap();
    let input = FaceInteraction::new(
        &face,
        &show,
        &action.identity,
        &action.target,
        vec![FaceInteractionArgument {
            name: "value".into(),
            value_kind: "value/bool".into(),
            value: vec![1],
        }],
        2,
    )
    .unwrap();
    let restored = data::decode_interaction(
        &data::encode_interaction(&InteractionRequest {
            revision: draft.revision(),
            basis: decoded.basis,
            show,
            input,
        })
        .unwrap(),
    )
    .unwrap();
    draft
        .apply_face_interaction(&restored.basis, &restored.show, &restored.input)
        .unwrap();
    assert!(draft.choices().iter().all(|choice| choice.selected));
}
#[test]
fn binary_transfer_does_not_accept_corruption_foreign_plan_or_stale_input() {
    let (mut draft, input) = fixture();
    let encoded = data::encode_interaction(&input).unwrap();
    assert!(data::decode_interaction(&encoded[..encoded.len() - 1]).is_err());
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(data::decode_interaction(&trailing).is_err());
    let mut wrong = data::decode_interaction(&encoded).unwrap();
    wrong.show.planned_mask.plan.plan_id = "plan/foreign".into();
    assert!(
        draft
            .apply_face_interaction(&wrong.basis, &wrong.show, &wrong.input)
            .is_err()
    );
    assert!(draft.choices().iter().all(|choice| choice.selected));
    let mut changed = data::decode_interaction(&encoded).unwrap();
    changed.show.planned_mask.plan.fragments[0].placements[0].implementation_id =
        "implementation/foreign".into();
    assert!(
        draft
            .apply_face_interaction(&changed.basis, &changed.show, &changed.input)
            .is_err()
    );
    assert!(draft.choices().iter().all(|choice| choice.selected));
    draft
        .apply_face_interaction(&input.basis, &input.show, &input.input)
        .unwrap();
    assert!(
        draft
            .apply_face_interaction(&input.basis, &input.show, &input.input)
            .is_err()
    );
    assert_eq!(
        draft
            .choices()
            .iter()
            .filter(|choice| choice.selected)
            .count(),
        3
    );
}
#[test]
fn public_json_mask_transport_stays_lossless_compressed_and_distinct() {
    let (_, input) = fixture();
    let json = serde_json::to_vec(&input).unwrap();
    assert!(
        std::str::from_utf8(&json)
            .unwrap()
            .contains("conduit.face/mask-plan-deflate-json@1")
    );
    let decoded: InteractionRequest = serde_json::from_slice(&json).unwrap();
    assert_eq!(decoded.show, input.show);
    assert!(data::decode_interaction(&json).is_err());
}

#[test]
fn extracted_codec_retains_previous_named_map_bytes_and_refusal_boundaries() {
    // Independent pre-extraction outer schema and serializer oracle. References
    // serialize transparently; all complete nested Plan/Show values are kept.
    #[derive(serde::Serialize)]
    struct PreviousRequest<'a> {
        revision: u32,
        basis: &'a BirthFaceBasis,
        show: &'a conduit_face::MaskShow,
        input: &'a FaceInteraction,
    }
    let (_, input) = fixture();
    let previous = PreviousRequest {
        revision: input.revision,
        basis: &input.basis,
        show: &input.show,
        input: &input.input,
    };
    let mut expected = b"CDCI\x01".to_vec();
    ciborium::ser::into_writer(&previous, &mut expected).unwrap();
    assert_eq!(data::encode_interaction(&input).unwrap(), expected);
    use conduitos_creche_core::wire::{MAX_BLOB_BYTES, Refusal};
    assert_eq!(data::decode_interaction(&[]).err(), Some(Refusal::Bounds));
    assert_eq!(
        data::decode_interaction(&vec![0; MAX_BLOB_BYTES as usize + 1]).err(),
        Some(Refusal::Bounds)
    );
    let mut wrong_version = expected.clone();
    wrong_version[4] = 2;
    assert_eq!(
        data::decode_interaction(&wrong_version).err(),
        Some(Refusal::Malformed)
    );
    assert_eq!(
        data::decode_interaction(b"CDCI\x01").err(),
        Some(Refusal::Malformed)
    );
    // CBOR tags are ignored by the original typed decoder. This exact-cap
    // well-formed carrier proves bounds without relying on an invalid payload.
    let mut exact = b"CDCI\x01".to_vec();
    exact.resize(MAX_BLOB_BYTES as usize - expected.len() + 5, 0xc0);
    exact.extend_from_slice(&expected[5..]);
    assert_eq!(exact.len(), MAX_BLOB_BYTES as usize);
    assert_eq!(data::decode_interaction(&exact).unwrap().show, input.show);
    exact.push(0);
    assert_eq!(
        data::decode_interaction(&exact).err(),
        Some(Refusal::Bounds)
    );
    let mut huge = input;
    huge.show.show.front_subject = "x".repeat(MAX_BLOB_BYTES as usize);
    assert_eq!(
        data::encode_interaction(&huge).err(),
        Some(Refusal::Capacity)
    );
}

#[test]
fn extracted_codec_refuses_unknown_duplicate_and_missing_named_fields() {
    use ciborium::value::Value;
    use conduitos_creche_core::wire::Refusal;
    let (_, input) = fixture();
    let original = data::encode_interaction(&input).unwrap();
    let value: Value = ciborium::de::from_reader(&original[5..]).unwrap();
    let Value::Map(fields) = value else {
        panic!("named map required")
    };
    let mut unknown = fields.clone();
    unknown.push((Value::Text("hidden_grant".into()), Value::Bool(true)));
    let mut duplicate = fields.clone();
    duplicate.push(
        fields
            .iter()
            .find(|(key, _)| *key == Value::Text("revision".into()))
            .unwrap()
            .clone(),
    );
    let missing = fields
        .into_iter()
        .filter(|(key, _)| *key != Value::Text("revision".into()))
        .collect();
    for fields in [unknown, duplicate, missing] {
        let mut bytes = b"CDCI\x01".to_vec();
        ciborium::ser::into_writer(&Value::Map(fields), &mut bytes).unwrap();
        assert_eq!(
            data::decode_interaction(&bytes).err(),
            Some(Refusal::Malformed)
        );
    }
}

#[test]
fn extracted_codec_keeps_actual_typed_recursion_ceiling() {
    use conduit_core::*;
    let (_, mut input) = fixture();
    let mut nested = input.show.planned_mask.plan.clone();
    nested.fragments.clear();
    nested.activations.clear();
    nested.activation_preparations.clear();
    let front = PlannedActivationFront {
        front_port_id: "fixture/front".into(),
        value_kind: "value/bool".into(),
        abnormal_kind: None,
    };
    for _ in 0..20 {
        let mut parent = nested.clone();
        parent.activations.clear();
        parent
            .activations
            .push(PlannedActivationEntry::Unary(PlannedActivation {
                activation_id: "fixture/nested".into(),
                owner_placement_id: "placement/fixture".into(),
                selected_plan_id: nested.plan_id.clone(),
                selected_plan: Box::new(nested),
                input: front.clone(),
                output: front.clone(),
                limits: PlannedActivationLimits {
                    maximum_active: 1,
                    maximum_queue_items: 1,
                    maximum_queue_bytes: 1,
                    maximum_items: 1,
                },
                terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
                cancellation_policy:
                    PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
                effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
                per_activation_sign_budget: SignStorageBudget {
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            }));
        nested = parent;
    }
    input.show.planned_mask.plan = nested;
    let bytes = data::encode_interaction(&input).unwrap();
    // This is structurally typed input, not an admitted executable Plan. A high
    // reference limit parses it, while the real private limit must refuse it.
    let _: InteractionRequest =
        ciborium::de::from_reader_with_recursion_limit(&bytes[5..], 256).unwrap();
    assert_eq!(
        data::decode_interaction(&bytes).err(),
        Some(conduitos_creche_core::wire::Refusal::Malformed)
    );
}
