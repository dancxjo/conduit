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
fn wire_roundtrip(input: &InteractionRequest) -> InteractionRequest {
    data::decode_interaction(&data::encode_interaction(input).unwrap()).unwrap()
}

#[test]
fn packed_transfer_preserves_the_complete_reference_plan_and_canonical_checks() {
    let (_, input) = fixture();
    let mut original = b"CDCI\x01".to_vec();
    ciborium::ser::into_writer(&input, &mut original).unwrap();
    let mut remaining = &original[5..];
    let reference: InteractionRequest =
        ciborium::de::from_reader_with_recursion_limit(&mut remaining, 64).unwrap();
    assert!(remaining.is_empty());

    let packed = data::encode_interaction(&input).unwrap();
    assert_eq!(&packed[..5], b"CDCI\x02");
    // This minimal fixture contains only a one-byte zero argument: packing
    // preserves its two-byte width; real Mask configuration arrays shrink.
    assert!(packed.len() <= original.len());
    let decoded = data::decode_interaction(&packed).unwrap();
    assert_eq!(decoded.revision, reference.revision);
    assert_eq!(decoded.basis, reference.basis);
    assert_eq!(decoded.show, reference.show);
    assert_eq!(decoded.input, reference.input);
    assert!(conduit_core::verify_plan(&reference.show.planned_mask.plan));
    assert!(conduit_core::verify_plan(&decoded.show.planned_mask.plan));
    assert!(data::decode_interaction(&original).is_err());
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
    let wrong = wire_roundtrip(&wrong);
    assert!(
        draft
            .apply_face_interaction(&wrong.basis, &wrong.show, &wrong.input)
            .is_err()
    );
    assert!(draft.choices().iter().all(|choice| choice.selected));
    let mut changed = data::decode_interaction(&encoded).unwrap();
    changed.show.planned_mask.plan.fragments[0].placements[0].implementation_id =
        "implementation/foreign".into();
    let changed = wire_roundtrip(&changed);
    assert!(
        draft
            .apply_face_interaction(&changed.basis, &changed.show, &changed.input)
            .is_err()
    );
    assert!(draft.choices().iter().all(|choice| choice.selected));
    let first = data::decode_interaction(&encoded).unwrap();
    draft
        .apply_face_interaction(&first.basis, &first.show, &first.input)
        .unwrap();
    let replay = data::decode_interaction(&encoded).unwrap();
    assert!(
        draft
            .apply_face_interaction(&replay.basis, &replay.show, &replay.input)
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
fn packed_transfer_still_invokes_the_structured_configuration_constructor() {
    use conduit_core::{
        ConfigurationEntry, ConfigurationValue, StructuredConfigurationValue, StructuredInfoType,
        StructuredInfoValue, TEXT_INFO_ID, kind_id,
    };
    let (_, mut input) = fixture();
    let ty = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let value = StructuredInfoValue::leaf(ty, b"codec constructor fixture".to_vec()).unwrap();
    let structured =
        StructuredConfigurationValue::new(profile.clone(), value.canonical_bytes().unwrap())
            .unwrap();
    input.show.planned_mask.plan.fragments[0].placements[0]
        .configuration
        .push(ConfigurationEntry {
            key: "codec-constructor-fixture".into(),
            value: ConfigurationValue::Structured(structured),
        });
    // The deliberately edited Plan need not be executable to test transport:
    // the genuine structured value must survive its own checked constructor.
    let mut encoded = data::encode_interaction(&input).unwrap();
    assert!(data::decode_interaction(&encoded).is_ok());
    let profile_bytes = profile.as_str().as_bytes();
    let offsets = encoded
        .windows(profile_bytes.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == profile_bytes).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "fixture profile must be unambiguous");
    // Preserve CBOR framing and text length while forging the claimed profile.
    encoded[offsets[0]] = b'x';
    assert_eq!(
        data::decode_interaction(&encoded).err(),
        Some(conduitos_creche_core::wire::Refusal::Malformed)
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
