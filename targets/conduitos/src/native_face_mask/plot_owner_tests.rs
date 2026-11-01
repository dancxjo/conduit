//! Actual native Source/Body Plan/start plus CPU Mask/ACK enactment. These tests
//! do not claim a Linux window, QEMU scanout, physical display or make effect.
use super::*;
use alloc::vec;
use conduit_face::{
    FaceAction, FaceActionAvailability, FaceBasis, FaceDisclosure, FaceDisclosureLevel,
    FaceInteraction, FaceRelationship, FaceSubject, FaceText, ManifestationLifecycle,
};

fn fixture() -> (
    NativeFaceMask,
    BodyPlan,
    BodyPlayIdentity,
    ResidentPlot,
    crate::native_workset::NativeWorksetPlay,
) {
    let plot = crate::native_workset::NativePlot::MemoryLantern;
    let wake = crate::native_workset::tests::wake(&[plot]);
    let (ids, offer) = crate::native_workset::tests::fixture();
    let prepared = crate::native_workset::prepare(&wake, &ids, &offer, "build").unwrap();
    let plan = prepared.plan().clone();
    let active = BodyPlayIdentity::bind(&plan, 1);
    let mut play = crate::native_workset::NativeWorksetPlay::prepare(&prepared).unwrap();
    play.start_for(&plan, &active).unwrap();
    let local = &plan.plots[0].plan.fragments[0];
    let provider = crate::product_bases::fixture_surface_provider();
    let mask = NativeFaceMask::prepare(
        local.host_id.clone(),
        local.boot_id.clone(),
        local.offer_generation,
        "build",
        provider.entry.base_id.clone(),
        "surface/plot-owner",
        &provider,
    )
    .unwrap();
    let resident = crate::native_workset::resident(plot).unwrap();
    (mask, plan, active, resident, play)
}
fn face(plan: &BodyPlan, play: &BodyPlayIdentity, plot: &ResidentPlot, revision: u64) -> Face {
    let source = &plan.plots.iter().find(|p| &p.plot == plot).unwrap().plan;
    Face::new_with_semantics(
        revision,
        FaceBasis {
            body_id: Some(plan.body_id.clone()),
            wake_id: Some(plan.wake_id.clone()),
            source_document_id: Some(source.source_document_id.clone()),
            checked_plot_id: Some(source.checked_plot_id.clone()),
            expanded_plot_id: Some(source.expanded_plot_id.clone()),
            plan_id: Some(source.plan_id.clone()),
            active_play_id: Some(play.active_play_id.clone()),
            sign_ids: vec![],
        },
        vec![
            FaceSubject {
                identity: "encounter/plot".into(),
                role: FaceRole::Plot,
                name: "Crèche".into(),
            },
            FaceSubject {
                identity: "encounter/plot/article".into(),
                role: FaceRole::Semantic(conduit_core::kind_id("document/article")),
                name: "Make its machinery".into(),
            },
        ],
        vec![FaceRelationship {
            source: "encounter/plot".into(),
            target: "encounter/plot/article".into(),
            kind: FaceRelationshipKind::Contains,
        }],
        vec![],
        vec![FaceText {
            subject: "encounter/plot/article".into(),
            text: "Review another Body's image preparation".into(),
        }],
        vec![FaceAction {
            identity: "encounter.review".into(),
            intent: "conduit.intent/review@1".into(),
            target: "encounter/plot".into(),
            name: "Review image preparation".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::Primary,
            availability: FaceActionAvailability::Available,
        }],
        vec![FaceDisclosure {
            subject: "encounter/plot".into(),
            level: FaceDisclosureLevel::Primary,
        }],
    )
    .unwrap()
}

#[test]
fn plot_owned_face_preserves_exact_basis_through_ack_and_typed_interaction() {
    let (mut mask, plan, play, plot, _live) = fixture();
    let original = face(&plan, &play, &plot, 1);
    assert!(
        original
            .subjects
            .iter()
            .all(|s| s.role != FaceRole::Host && s.role != FaceRole::Body)
    );
    let mut display = crate::native_face_mask::tests::CountingDisplay::default();
    let composition = mask
        .present_plot_owner(original.clone(), &plan, &play, &plot, 1, 1, &mut display)
        .unwrap();
    assert_eq!(composition.face_id, original.identity);
    assert_eq!(
        mask.publication().unwrap().observed_face_basis,
        original.basis
    );
    let show = mask.show().unwrap().clone();
    assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
    assert_eq!(show.show.face_id, original.identity);
    let action = &original.actions[0];
    let interaction = FaceInteraction::new(
        &original,
        &show,
        &action.identity,
        &action.target,
        vec![],
        1,
    )
    .unwrap();
    let correlation = mask.submit(interaction.clone()).unwrap();
    assert_eq!(correlation.interaction, interaction);
    assert_eq!(correlation.show_id, show.show_id);
}

#[test]
fn forged_producer_or_stale_execution_never_replaces_current_show() {
    let (mut mask, plan, play, plot, _live) = fixture();
    let original = face(&plan, &play, &plot, 1);
    let mut display = crate::native_face_mask::tests::CountingDisplay::default();
    mask.present_plot_owner(original.clone(), &plan, &play, &plot, 1, 1, &mut display)
        .unwrap();
    let show = mask.show().unwrap().show_id.clone();
    let mut invalid = vec![];
    let mut basis = original.basis.clone();
    basis.source_document_id = Some("source/forged".into());
    invalid.push(basis);
    let mut basis = original.basis.clone();
    basis.checked_plot_id = Some("checked/forged".into());
    invalid.push(basis);
    let mut basis = original.basis.clone();
    basis.expanded_plot_id = Some("expanded/forged".into());
    invalid.push(basis);
    let mut basis = original.basis.clone();
    basis.plan_id = Some(plan.plan_id.clone());
    invalid.push(basis); // Body Plan cannot replace producing Plot Plan.
    let mut basis = original.basis.clone();
    basis.body_id = None;
    basis.wake_id = None;
    invalid.push(basis);
    let mut basis = original.basis.clone();
    basis.wake_id = None;
    invalid.push(basis);
    let mut basis = original.basis.clone();
    basis.active_play_id = Some("play/stale".into());
    invalid.push(basis);
    for basis in invalid {
        let forged = original.clone().with_basis(basis).unwrap();
        assert!(
            mask.present_plot_owner(forged, &plan, &play, &plot, 2, 2, &mut display)
                .is_err()
        );
        assert_eq!(mask.show().unwrap().show_id, show);
    }
    assert!(
        mask.present_plot_owner(
            original.clone(),
            &plan,
            &BodyPlayIdentity::bind(&plan, 2),
            &plot,
            2,
            2,
            &mut display
        )
        .is_err()
    );
    let action = &original.actions[0];
    let old = FaceInteraction::new(
        &original,
        mask.show().unwrap(),
        &action.identity,
        &action.target,
        vec![],
        1,
    )
    .unwrap();
    let fresh = face(&plan, &play, &plot, 2);
    mask.present_plot_owner(fresh, &plan, &play, &plot, 2, 2, &mut display)
        .unwrap();
    assert!(mask.submit(old).is_err());
    assert!(mask.show().is_some());
}

#[test]
fn plot_role_alone_and_an_unrelated_mask_host_are_insufficient() {
    let (mut mask, plan, play, plot, _live) = fixture();
    let original = face(&plan, &play, &plot, 1);
    let mut display = crate::native_face_mask::tests::CountingDisplay::default();
    assert!(mask.present(original.clone(), 1, 1, &mut display).is_err());
    let (mut unrelated, _) = crate::native_face_mask::tests::fixture();
    assert!(
        unrelated
            .present_plot_owner(original, &plan, &play, &plot, 1, 1, &mut display)
            .is_err()
    );
    assert!(unrelated.show().is_none());
}
