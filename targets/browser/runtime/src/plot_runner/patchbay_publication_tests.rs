//! Actual Mask/ingress kernels with local Host acknowledgement fixtures only.
use super::super::*;
use crate::patchbay_ingress::*;
use conduit_face::*;
use patchbay_application::{PatchbayApplicationRequest, EDIT_CURRENT_ACTION_ID};

#[test]
fn prepared_mask_show_and_typed_fore_reach_current_source_owner_once() {
    let source = include_str!("../../../../../plots/patchbay/main.conduit");
    let mut request = body_start::tests::request_from_sources(&[source]);
    request.source = source.into();
    let play = conduit_body::BodyPlayIdentity::bind(&request.plan, request.play_sequence);
    request.wake = request
        .wake
        .body_plan_ready(&request.plan, "sign/publication-plan-ready".into())
        .and_then(|wake| {
            wake.body_play_started(&request.plan, &play, "sign/publication-play-started".into())
        })
        .unwrap();
    let part = &request.plan.plots[0];
    let placement = part.plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::RETAINED_APPLICATION_KIND)
        .unwrap();
    let Some(application_state::PreparedApplication::Patchbay(application)) =
        application_state::PreparedApplication::prepare(
            placement,
            &request.plan,
            &play.active_play_id,
            request.body_evidence.as_ref(),
            source,
            &request.foreground_checked_plot_id,
        )
        .unwrap()
    else {
        panic!("exact Patchbay owner was not prepared");
    };
    let mut owner = application.port;
    let basis = FaceContributionBasis {
        checked_plot_id: part.plot.checked_plot_id.clone(),
        plan_id: request.plan.plan_id.clone(),
        active_play_id: play.active_play_id,
        required_interaction_context: None,
    };
    let (mut publication, effect, face) = publication_for(&request, &basis, &owner, 19);
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == "patchbay.plot.0")
        .unwrap();
    let proposed = BrowserMaskInteraction {
        show_id: effect.show_id.clone(),
        face_id: effect.face_id.clone(),
        face_revision: effect.face_revision,
        action_id: action.identity.clone(),
        target: action.target.clone(),
        arguments: vec![],
        sequence: 1,
    };
    let initial_revision = owner.revision();
    assert!(!publication.can_submit());
    assert!(publication.interact(&mut owner, &proposed).is_err());
    let ack = BrowserMaskAcknowledgement {
        show_id: effect.show_id,
        manifestation_id: effect.manifestation_id,
        mask_plan_id: effect.mask_plan_id,
        active_play_id: effect.mask_play.active_play_id,
        placement_id: effect.placement_id,
        face_id: effect.face_id,
        face_revision: effect.face_revision,
    };
    let mut stale = ack.clone();
    stale.face_revision += 1;
    assert!(publication.acknowledge(&stale).is_err());
    assert_eq!(owner.revision(), initial_revision);
    publication.acknowledge(&ack).unwrap();
    assert!(publication.can_submit());
    let mut forged = proposed.clone();
    forged.show_id = "show/foreign".into();
    assert!(publication.interact(&mut owner, &forged).is_err());
    assert_eq!(owner.revision(), initial_revision);
    let receipt = publication.interact(&mut owner, &proposed).unwrap();
    assert_eq!(receipt.owner_result, Ok(None));
    assert!(receipt.transport_completion.is_ok());
    assert_eq!(owner.revision(), initial_revision + 1);
    let observed = publication.observation();
    observed
        .interaction
        .unwrap()
        .correlation
        .interaction
        .validate_against(&face, &observed.mask_show)
        .unwrap();
    assert!(!publication.can_submit());
    assert!(publication.interact(&mut owner, &proposed).is_err());
    publication.retire().unwrap();
    assert!(publication.take_receipt().unwrap().owner_result.is_ok());
    assert!(publication.take_receipt().is_none());
    assert!(publication.acknowledge(&ack).is_err());
    assert_eq!(owner.revision(), initial_revision + 1);
    let (mut edited, edit_effect, edit_face) = publication_for(&request, &basis, &owner, 20);
    let action = edit_face
        .actions
        .iter()
        .find(|a| a.identity == EDIT_CURRENT_ACTION_ID)
        .unwrap();
    let subject = owner
        .graph()
        .subject_identities()
        .next()
        .unwrap()
        .to_owned();
    let proposed_edit = BrowserMaskInteraction {
        show_id: edit_effect.show_id.clone(),
        face_id: edit_effect.face_id.clone(),
        face_revision: edit_effect.face_revision,
        action_id: action.identity.clone(),
        target: action.target.clone(),
        arguments: vec![FaceInteractionArgument {
            name: "subject".into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            value: subject.as_bytes().to_vec(),
        }],
        sequence: 1,
    };
    edited
        .acknowledge(&BrowserMaskAcknowledgement {
            show_id: edit_effect.show_id,
            manifestation_id: edit_effect.manifestation_id,
            mask_plan_id: edit_effect.mask_plan_id,
            active_play_id: edit_effect.mask_play.active_play_id,
            placement_id: edit_effect.placement_id,
            face_id: edit_effect.face_id,
            face_revision: edit_effect.face_revision,
        })
        .unwrap();
    assert_ne!(face.identity, edit_face.identity);
    let receipt = edited.interact(&mut owner, &proposed_edit).unwrap();
    assert_eq!(
        receipt.owner_result,
        Ok(Some(PatchbayApplicationRequest::EditCurrent {
            expanded_plot_id: owner.graph().expanded_plot_id.clone(),
            subject_identity: subject,
        }))
    );
    assert!(receipt.transport_completion.is_ok());
    assert_eq!(owner.revision(), initial_revision + 2);
    assert!(publication.interact(&mut owner, &proposed).is_err());
    edited.retire().unwrap();
    assert!(edited.take_receipt().unwrap().owner_result.is_ok());
}

fn publication_for(
    request: &body_start::BodyStartRequest,
    basis: &FaceContributionBasis,
    owner: &patchbay_application::PatchbayApplicationPort,
    sequence: u64,
) -> (
    PatchbayPublication,
    crate::workspace_mask::BrowserMaskEffect,
    Face,
) {
    let fragment = owner
        .face_fragment(basis.clone(), &request.plan.body_id, true)
        .unwrap();
    let face = BodyFace::project(
        &request.body_evidence.as_ref().unwrap().body,
        Some(&request.wake),
        u64::from(owner.revision()),
        FaceContext::ResidentPlot(basis.checked_plot_id.clone()),
        FaceFocus::Contribution {
            role: FaceContributionRole::Foreground,
            node_key: None,
        },
        vec![FaceContribution::from_face(
            FaceContributionRole::Foreground,
            fragment,
        )],
    )
    .unwrap()
    .face;
    let (mask, effect) = BrowserMaskRuntime::prepare_sequenced(
        request.plan.body_id.clone(),
        request.local_host_id.clone(),
        request.local_boot_id.clone(),
        face.clone(),
        request.wake.clone(),
        request.plan.clone(),
        sequence,
    )
    .unwrap();
    let mut host = crate::installed_browser::advertisement(
        request.local_host_id.clone(),
        request.local_boot_id.clone(),
    );
    host.capabilities.push(offer());
    let ingress =
        PreparedPatchbayIngress::prepare(PlannedPatchbayIngress::prepare(&host).unwrap()).unwrap();
    (
        PatchbayPublication::prepare(mask, ingress, basis.clone()).unwrap(),
        effect,
        face,
    )
}
