//! Actual Mask/ingress kernels with local Host acknowledgement fixtures only.
use super::super::*;
use crate::patchbay_ingress::*;
use conduit_face::*;
use patchbay_application::EDIT_CURRENT_ACTION_ID;

#[test]
fn prepared_mask_show_and_typed_fore_reach_current_source_owner_once() {
    let source = include_str!("../../../../../plots/patchbay/native-retained.conduit");
    let mut request = body_start::tests::request_from_sources(&[source]);
    request.source = source.into();
    seal_publication_masks(&mut request);
    let play = conduit_body::BodyPlayIdentity::bind(&request.plan, request.play_sequence);
    let prepared_request = serde_json::from_value(serde_json::to_value(&request).unwrap()).unwrap();
    let (mut session, started) = body_start::prepare(prepared_request).unwrap();
    request.wake = started.wake_at_start;
    let node = session.patchbay_face.as_ref().unwrap().owner_node;
    let budget = session
        .scheduler
        ._prepared_storage
        .publication_value_budget
        .as_ref()
        .unwrap();
    let Some(application_state::PreparedApplication::Patchbay(owner)) =
        session.scheduler.applications[usize::from(node.0)].as_deref_mut()
    else {
        panic!("exact scheduler Patchbay owner is not prepared");
    };
    let part = &request.plan.plots[0];
    let basis = FaceContributionBasis {
        checked_plot_id: part.plot.checked_plot_id.clone(),
        plan_id: request.plan.plan_id.clone(),
        active_play_id: play.active_play_id,
        required_interaction_context: None,
    };
    let prepared = session
        .patchbay_publication
        .take()
        .expect("Body preparation installed its captured Mask route");
    let mut publication = prepared.publication;
    let effect = prepared.effect;
    let face = publication.observation().face;
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
    let initial_revision = owner.port.revision();
    assert!(!publication.can_submit());
    assert!(publication.interact(owner, &proposed).is_err());
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
    assert_eq!(owner.port.revision(), initial_revision);
    publication.acknowledge(&ack).unwrap();
    assert!(publication.can_submit());
    let mut forged = proposed.clone();
    forged.show_id = "show/foreign".into();
    assert!(publication.interact(owner, &forged).is_err());
    assert_eq!(owner.port.revision(), initial_revision);
    let receipt = publication.interact(owner, &proposed).unwrap();
    assert_eq!(receipt.owner_result, Ok(None));
    assert!(receipt.transport_completion.is_ok());
    assert_eq!(owner.port.revision(), initial_revision + 1);
    let observed = publication.observation();
    observed
        .interaction
        .unwrap()
        .correlation
        .interaction
        .validate_against(&face, &observed.mask_show)
        .unwrap();
    assert!(!publication.can_submit());
    assert!(publication.interact(owner, &proposed).is_err());
    publication.retire().unwrap();
    assert!(publication.take_receipt().unwrap().owner_result.is_ok());
    assert!(publication.take_receipt().is_none());
    assert!(publication.acknowledge(&ack).is_err());
    assert_eq!(owner.port.revision(), initial_revision + 1);
    drop(publication); // Release its actual buffers before admitting a replacement.
    let (mut edited, edit_effect, edit_face) =
        publication_for(&request, &basis, &owner.port, budget, 20);
    let action = edit_face
        .actions
        .iter()
        .find(|a| a.identity == EDIT_CURRENT_ACTION_ID)
        .unwrap();
    let subject = owner
        .port
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
    let receipt = edited.interact(owner, &proposed_edit).unwrap();
    assert_eq!(receipt.owner_result, Ok(None));
    assert_eq!(
        receipt.request_delivery,
        PatchbayRequestDelivery::Retained {
            application_revision: initial_revision + 2,
        }
    );
    assert!(receipt.transport_completion.is_ok());
    assert_eq!(owner.port.revision(), initial_revision + 2);
    let pending = owner
        .pending_observation()
        .expect("canonical Edit retained for Host disposition");
    assert_eq!(pending["application_revision"], initial_revision + 2);
    assert_eq!(pending["request"]["kind"], "EditCurrent");
    assert_eq!(pending["request"]["subject_identity"], subject);
    assert_eq!(
        pending["request"]["expanded_plot_id"],
        owner.port.graph().expanded_plot_id.as_str()
    );
    let (mut blocked, blocked_effect, blocked_face) =
        publication_for(&request, &basis, &owner.port, budget, 21);
    blocked
        .acknowledge(&BrowserMaskAcknowledgement {
            show_id: blocked_effect.show_id,
            manifestation_id: blocked_effect.manifestation_id,
            mask_plan_id: blocked_effect.mask_plan_id,
            active_play_id: blocked_effect.mask_play.active_play_id,
            placement_id: blocked_effect.placement_id,
            face_id: blocked_effect.face_id,
            face_revision: blocked_effect.face_revision,
        })
        .unwrap();
    let observed = blocked.observation();
    let blocked_input = BrowserMaskInteraction {
        show_id: observed.mask_show.show_id.as_str().into(),
        face_id: blocked_face.identity.as_str().into(),
        face_revision: blocked_face.revision,
        action_id: proposed_edit.action_id.clone(),
        target: proposed_edit.target.clone(),
        arguments: proposed_edit.arguments.clone(),
        sequence: 1,
    };
    assert!(blocked.interact(owner, &blocked_input).is_err());
    assert!(
        blocked.can_submit(),
        "pressure must not consume the closing Mask input"
    );
    assert_eq!(owner.pending_observation().unwrap(), pending);
    assert_eq!(owner.port.revision(), initial_revision + 2);
    session.scheduler.cancel().unwrap();
    assert!(
        !blocked.can_submit(),
        "parent cancellation revokes the live publication"
    );
    blocked.retire().unwrap();

    edited.retire().unwrap();
    assert!(edited.take_receipt().unwrap().owner_result.is_ok());
}

fn publication_for(
    request: &body_start::BodyStartRequest,
    basis: &FaceContributionBasis,
    owner: &patchbay_application::PatchbayApplicationPort,
    budget: &PatchbayValueStorageBudget,
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
    let (mask, effect) = BrowserMaskRuntime::prepare_in_body_plan(
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
    assert_eq!(mask.observation().body_plan, request.plan);
    let ingress =
        PreparedPatchbayIngress::prepare(PlannedPatchbayIngress::prepare(&host).unwrap()).unwrap();
    (
        PatchbayPublication::prepare(mask, ingress, basis.clone(), budget).unwrap(),
        effect,
        face,
    )
}

fn seal_publication_masks(request: &mut body_start::BodyStartRequest) {
    let host = crate::installed_browser::membership_advertisement(
        request.local_host_id.clone(),
        request.local_boot_id.clone(),
    );
    let masks = crate::workspace_runtime::patchbay_planning::masks(
        &request.plan.plots,
        &[host],
        &request.local_host_id,
        &request.local_boot_id,
    )
    .unwrap();
    assert_eq!(masks.len(), 1);
    request.plan =
        conduit_body::BodyPlan::seal_with_masks(&request.wake, request.plan.plots.clone(), masks)
            .unwrap();
}

#[test]
fn resident_command_entry_points_require_acknowledgement_and_consume_one_show() {
    let source = include_str!("../../../../../plots/patchbay/main.conduit");
    let mut request = body_start::tests::request_from_sources(&[source]);
    request.source = source.into();
    seal_publication_masks(&mut request);
    let input_plan = request.plan.plots[0].plan.plan_id.clone();
    let (mut session, started) = body_start::prepare(request).unwrap();
    assert!(matches!(started.progress, TourProgress::Waiting { .. }));
    assert!(session
        .patchbay_publication
        .as_ref()
        .unwrap()
        .publication
        .is_resident());
    let capacities = session.scheduler.values().allocation_capacities();
    let owner_node = session.patchbay_face.as_ref().unwrap().owner_node;
    let Some(application_state::PreparedApplication::Patchbay(owner)) = session
        .scheduler
        .applications
        .get_mut(usize::from(owner_node.0))
        .and_then(|app| app.as_deref_mut())
    else {
        panic!("owner")
    };
    let revision = owner.port.revision();
    assert_eq!(
        owner.execute(&[255]).unwrap_err(),
        "patchbay-canonical-interaction-required"
    );
    assert_eq!(owner.port.revision(), revision);
    assert!(owner.execute(&[]).is_ok());
    assert!(session.scheduler.has_patchbay_fore(owner_node));
    let before = session.patchbay_observation().unwrap();
    assert_eq!(before["publication"]["interaction_admitted"], false);
    assert_eq!(before["publication"]["view"]["show_state"], "prepared");
    assert_eq!(
        before["publication"]["view"]["interactions_admitted"],
        false
    );
    let prepared = session.patchbay_publication.as_ref().unwrap();
    let effect = prepared.effect.clone();
    let face = prepared.publication.observation().face;
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == "patchbay.plot.0")
        .unwrap()
        .clone();
    let input = || BrowserMaskInteraction {
        show_id: effect.show_id.clone(),
        face_id: effect.face_id.clone(),
        face_revision: effect.face_revision,
        action_id: action.identity.clone(),
        target: action.target.clone(),
        arguments: vec![],
        sequence: 1,
    };
    assert!(session.patchbay_interact(input()).is_err());
    let acknowledgement = || BrowserMaskAcknowledgement {
        show_id: effect.show_id.clone(),
        manifestation_id: effect.manifestation_id.clone(),
        mask_plan_id: effect.mask_plan_id.clone(),
        active_play_id: effect.mask_play.active_play_id.clone(),
        placement_id: effect.placement_id.clone(),
        face_id: effect.face_id.clone(),
        face_revision: effect.face_revision,
    };
    let mut stale = acknowledgement();
    stale.face_revision += 1;
    assert!(session.patchbay_acknowledge(stale).is_err());
    let available = session.patchbay_acknowledge(acknowledgement()).unwrap();
    assert_eq!(available["publication"]["interaction_admitted"], true);
    assert_eq!(available["publication"]["view"]["show_state"], "available");
    assert_eq!(
        available["publication"]["view"]["interactions_admitted"],
        true
    );
    let accepted = session.patchbay_interact(input()).unwrap();
    assert_eq!(
        accepted["publication"]["dispatch"]["ingress_plan_id"],
        input_plan.as_str()
    );
    assert_eq!(
        session.scheduler.values().allocation_capacities(),
        capacities
    );
    let Some(application_state::PreparedApplication::Patchbay(owner)) = session
        .scheduler
        .applications
        .get_mut(usize::from(owner_node.0))
        .and_then(|app| app.as_deref_mut())
    else {
        panic!("owner")
    };
    let observation = ApplicationView::decode(&owner.execute(&[]).unwrap()).unwrap();
    assert!(observation
        .nodes
        .iter()
        .all(|node| node.component != ApplicationComponent::PatchbayCanvas));
    let revision = owner.port.revision();
    assert!(owner.execute(&[255]).is_err());
    assert_eq!(owner.port.revision(), revision);

    assert_eq!(accepted["publication"]["dispatch"]["owner_accepted"], true);
    assert_eq!(
        accepted["publication"]["dispatch"]["transport_completed"],
        true
    );
    assert_eq!(accepted["publication"]["interaction_admitted"], false);
    assert_eq!(
        accepted["publication"]["view"]["interactions_admitted"],
        false
    );
    assert_eq!(
        accepted["face"]["revision"].as_u64(),
        before["face"]["revision"]
            .as_u64()
            .map(|revision| revision + 1)
    );
    assert!(session.patchbay_interact(input()).is_err());
    assert_eq!(session.patchbay_observation().unwrap(), accepted);
    let stale_target = publication_target(&session);
    let refreshed = session.patchbay_refresh(stale_target.clone()).unwrap();
    assert_ne!(
        refreshed["publication"]["effect"]["show_id"],
        effect.show_id
    );
    assert_eq!(
        refreshed["publication"]["dispatch"]["show_id"],
        effect.show_id
    );
    assert_eq!(refreshed["publication"]["interaction_admitted"], false);
    assert!(session.patchbay_refresh(stale_target).is_err());
    assert!(session.patchbay_acknowledge(acknowledgement()).is_err());
    let prepared = session.patchbay_publication.as_ref().unwrap();
    let fresh_effect = prepared.effect.clone();
    let fresh_face = prepared.publication.observation().face;
    let edit = fresh_face
        .actions
        .iter()
        .find(|action| action.identity == EDIT_CURRENT_ACTION_ID)
        .unwrap()
        .clone();
    let node = session.patchbay_face.as_ref().unwrap().owner_node;
    let Some(application_state::PreparedApplication::Patchbay(owner)) =
        session.scheduler.applications[usize::from(node.0)].as_deref()
    else {
        panic!("owner absent");
    };
    let subject = owner
        .port
        .graph()
        .subject_identities()
        .into_iter()
        .next()
        .unwrap()
        .to_string();
    session
        .patchbay_acknowledge(BrowserMaskAcknowledgement {
            show_id: fresh_effect.show_id.clone(),
            manifestation_id: fresh_effect.manifestation_id,
            mask_plan_id: fresh_effect.mask_plan_id,
            active_play_id: fresh_effect.mask_play.active_play_id,
            placement_id: fresh_effect.placement_id,
            face_id: fresh_effect.face_id.clone(),
            face_revision: fresh_effect.face_revision,
        })
        .unwrap();
    let edited = session
        .patchbay_interact(BrowserMaskInteraction {
            show_id: fresh_effect.show_id.clone(),
            face_id: fresh_effect.face_id,
            face_revision: fresh_effect.face_revision,
            action_id: edit.identity,
            target: edit.target,
            arguments: vec![FaceInteractionArgument {
                name: "subject".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: subject.as_bytes().to_vec(),
            }],
            sequence: 1,
        })
        .unwrap();
    assert_eq!(
        edited["pending_owner_request"]["request"]["kind"],
        "EditCurrent"
    );
    assert_eq!(
        edited["publication"]["dispatch"]["ingress_plan_id"],
        input_plan.as_str()
    );
    assert_eq!(
        session.scheduler.values().allocation_capacities(),
        capacities
    );
    let pending = edited["pending_owner_request"].clone();
    let target = publication_target(&session);
    let waiting = session.patchbay_refresh(target).unwrap();
    assert_eq!(waiting["pending_owner_request"], pending);
    assert_eq!(
        waiting["publication"]["dispatch"]["show_id"],
        fresh_effect.show_id
    );
    assert!(!session
        .patchbay_publication
        .as_ref()
        .unwrap()
        .publication
        .observation()
        .face
        .actions
        .iter()
        .any(|action| action.identity.starts_with("patchbay.")
            && action.availability == FaceActionAvailability::Available));
    assert!(session.patchbay_publication.as_ref().unwrap().publication.observation().face.actions.iter()
        .filter(|action| action.identity.starts_with("patchbay."))
        .all(|action| matches!(&action.availability, FaceActionAvailability::Unavailable { reason_code, .. }
            if reason_code == "patchbay-owner-request-pending")));
}

fn publication_target(session: &TourSession) -> super::super::patchbay_commands::PublicationTarget {
    let effect = &session.patchbay_publication.as_ref().unwrap().effect;
    super::super::patchbay_commands::PublicationTarget {
        body_plan_id: session.patchbay_face.as_ref().unwrap().plan.plan_id.clone(),
        active_play_id: session.active_play_id.clone(),
        show_id: effect.show_id.clone(),
        face_id: effect.face_id.clone(),
        face_revision: effect.face_revision,
    }
}

#[test]
fn resident_publication_refuses_an_unreserved_initial_mask_route() {
    let source = include_str!("../../../../../plots/patchbay/main.conduit");
    let mut request = body_start::tests::request_from_sources(&[source]);
    request.source = source.into();
    seal_publication_masks(&mut request);
    let mut masks = request.plan.mask_topologies.clone();
    masks[0].chains.reverse();
    request.plan =
        conduit_body::BodyPlan::seal_with_masks(&request.wake, request.plan.plots.clone(), masks)
            .unwrap();
    let error = body_start::prepare(request)
        .err()
        .expect("reversed initial route must refuse");
    assert!(format!("{error:?}").contains("initial browser Mask route differs"));
}
