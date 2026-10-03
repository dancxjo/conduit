extern crate std;

use super::*;
use alloc::vec;
use conduit_birth_plot::{BirthDraft, BirthFaceBasis, BirthPlotChoice};
use conduit_presentation::{
    Face, FaceContext, FaceFocus, PresentationBasis, PresentationRole, PresentationSubject,
    PresentationText,
};

fn producer() -> NativeFaceSnapshotProducer {
    NativeFaceSnapshotProducer::prepare(
        "host/native".into(),
        "boot/native".into(),
        OfferGeneration(1),
        "build",
    )
    .unwrap()
}

fn basis(producer: &NativeFaceSnapshotProducer) -> PresentationBasis {
    PresentationBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some(producer.plan.source_document_id.clone()),
        checked_plot_id: Some(producer.plan.checked_plot_id.clone()),
        expanded_plot_id: Some(producer.plan.expanded_plot_id.clone()),
        plan_id: Some(producer.plan.plan_id.clone()),
        active_play_id: None,
        sign_ids: vec![],
    }
}

fn arrival(producer: &NativeFaceSnapshotProducer) -> Presentation {
    // The real Birth projection and checked native inventory, with no invented
    // producer Plan or Body. This test executes the production forwarding path.
    let choices = crate::native_workset::profile()
        .installed()
        .iter()
        .map(|plot| BirthPlotChoice {
            title: plot.title().into(),
            search_text: plot.title().into(),
            plot: crate::native_workset::resident(*plot).unwrap(),
            refusal: None,
            selected: true,
        })
        .collect();
    BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices)
        .unwrap()
        .face(&BirthFaceBasis {
            host_id: "host/native".into(),
            boot_id: "boot/native".into(),
            encounter_id: "arrival/one".into(),
            producer_plot: producer.plot_identity(),
            producer_plan_id: producer.plan.plan_id.clone(),
        })
        .unwrap()
}

#[test]
fn real_zero_body_birth_face_crosses_exact_kernel_fore_without_changing_facts() {
    let mut producer = producer();
    let face = arrival(&producer);
    let bytes = serde_json::to_vec(&face).unwrap();
    std::println!("real native Birth Face: {} bytes", bytes.len());
    let published = producer.forward(face.clone(), 7, 11).unwrap();
    assert!(conduit_core::verify_plan(producer.plan()));
    assert_eq!(published.presentation, face);
    assert_eq!(published.receipt.observed_face_basis, face.basis);
    assert!(published.presentation.basis.body_id.is_none());
    assert!(published.presentation.basis.wake_id.is_none());
    assert!(published.presentation.basis.active_play_id.is_none());
    assert_eq!(published.receipt.observation_sequence, 7);
    assert_eq!(published.receipt.encoded_bytes as usize, bytes.len());
    assert_eq!(published.receipt.producer_plan_id, producer.plan.plan_id);
    assert_eq!(published.receipt.host_id.as_str(), "host/native");
    assert_eq!(published.receipt.boot_id.as_str(), "boot/native");
    assert_eq!(published.receipt.fore_endpoints, 2);
    assert!(published.receipt.kernel_signs > 0);
    let expected = conduit_core::bind_active_play(
        &producer.plan.plan_id,
        &"host/native".into(),
        &"boot/native".into(),
        11,
    );
    assert_eq!(
        published.receipt.producer_active_play_id,
        expected.active_play_id
    );
    assert!(
        published
            .presentation
            .actions
            .iter()
            .any(|action| action.identity == "creche.birth")
    );
}

#[test]
fn resting_body_snapshot_forwarding_does_not_wake_the_body() {
    let mut producer = producer();
    let body =
        conduit_body::Body::born_with_plots(conduit_body::BodyWorkset::default(), 1, "born".into())
            .unwrap();
    let face = Face::project(
        &body,
        None,
        2,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .presentation;
    let published = producer.forward(face.clone(), 2, 1).unwrap();
    assert_eq!(published.presentation, face);
    assert_eq!(published.receipt.observed_face_basis, face.basis);
    assert_eq!(
        published.presentation.basis.body_id,
        Some(body.body_id.clone())
    );
    assert!(published.presentation.basis.wake_id.is_none());
    assert!(published.presentation.basis.active_play_id.is_none());
    assert_eq!(body.state, conduit_body::BodyState::Lulled);
}

#[test]
fn another_plan_or_claimed_body_play_cannot_be_laundered_as_snapshot_provenance() {
    let mut producer = producer();
    let face = arrival(&producer);
    let mut stale = face.basis.clone();
    stale.plan_id = Some("plan/unrelated".into());
    assert!(matches!(
        producer.prepare_snapshot(face.clone().with_basis(stale).unwrap(), 1, 1),
        Err(FaceSnapshotRefusal::ProducerBasis)
    ));
    let mut live = face.basis.clone();
    live.active_play_id = Some("play/not-this-producer".into());
    assert!(matches!(
        producer.prepare_snapshot(face.with_basis(live).unwrap(), 1, 1),
        Err(FaceSnapshotRefusal::ProducerBasis)
    ));
}

#[test]
fn whole_valid_face_above_profile_capacity_is_refused_before_execution() {
    let mut producer = producer();
    let face = Presentation::new_with_semantics(
        1,
        basis(&producer),
        vec![PresentationSubject {
            identity: "host/snapshot".into(),
            role: PresentationRole::Host,
            name: "Host observation".into(),
        }],
        vec![],
        vec![],
        (0..32)
            .map(|_| PresentationText {
                subject: "host/snapshot".into(),
                text: "x".repeat(conduit_presentation::MAX_PRESENTATION_TEXT_BYTES),
            })
            .collect(),
        vec![],
        vec![],
    )
    .unwrap();
    assert!(serde_json::to_vec(&face).unwrap().len() > MAX_SNAPSHOT_BYTES);
    assert!(matches!(
        producer.prepare_snapshot(face, 1, 1),
        Err(FaceSnapshotRefusal::Capacity)
    ));
}

#[test]
fn prepared_kernel_can_cancel_without_publishing_or_changing_the_snapshot() {
    let mut producer = producer();
    let face = arrival(&producer);
    producer
        .prepare_snapshot(face.clone(), 1, 1)
        .unwrap()
        .cancel()
        .unwrap();
    assert!(matches!(
        producer.prepare_snapshot(face.clone(), 1, 1),
        Err(FaceSnapshotRefusal::PlaySequence)
    ));
    // Cancellation leaves no retained kernel or output attached to the producer.
    let published = producer.forward(face.clone(), 2, 2).unwrap();
    assert_eq!(published.presentation, face);
}

#[test]
fn stale_plan_seal_is_refused_before_kernel_preparation() {
    let mut producer = producer();
    let face = arrival(&producer);
    producer.plan.fragments[0].placements[0].implementation_id = "unadmitted/implementation".into();
    assert!(matches!(
        producer.prepare_snapshot(face, 1, 1),
        Err(FaceSnapshotRefusal::Plan)
    ));
}
