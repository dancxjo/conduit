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
