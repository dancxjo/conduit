use super::*;
use conduit_body::{BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology};

const SOURCE: &str = "plot note {\n value: text/literal(\"ready\")\n show: face/text(\"Message\")\n value >> show\n}\n";

fn prepared_session() -> TourSession {
    let mut request = super::super::body_start::tests::request_from_sources(&[SOURCE]);
    request.source = SOURCE.into();
    request.owner_face_revision = Some("0".into());
    let host = crate::installed_browser::membership_advertisement(
        request.local_host_id.clone(),
        request.local_boot_id.clone(),
    );
    let planned = crate::workspace_mask::plan::planned_mask(
        &host,
        crate::workspace_mask::plan::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    let chains = vec![BodyMaskChainPlan {
        stage_placement_ids: planned
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| placement.placement_id.clone())
            .collect(),
        plan: planned.plan,
    }];
    request.plan = BodyPlan::seal_with_masks(
        &request.wake,
        request.plan.plots.clone(),
        vec![BodyMaskTopology {
            face: BodyFaceSelector {
                plot: None,
                source_placement_id: None,
            },
            chains,
        }],
    )
    .unwrap();
    super::super::body_start::prepare(request).unwrap().0
}

fn request(session: &TourSession, revision: u64) -> BodyFaceRequest {
    let context = session.body_face_publication.as_ref().unwrap();
    let encoded = serde_json::to_vec(&context.evidence).unwrap();
    let attachment =
        BodyEvidenceAttachment::open_serialized(&encoded, BodyEvidenceEntrance::ExternalReader)
            .unwrap();
    BodyFaceRequest {
        schema: "conduit.browser/body-face-publication@1".into(),
        body_plan_id: context.plan.plan_id.clone(),
        active_play_id: session.active_play_id.clone(),
        revision,
        body_evidence: context.evidence.clone(),
        reviewed_plots: context.reviewed.clone(),
        face: conduit_body_make::project_body_workbench_face(
            revision,
            &attachment,
            &context.reviewed,
        )
        .unwrap(),
    }
}

fn ack(value: &serde_json::Value) -> BrowserMaskAcknowledgement {
    let effect = &value["effect"];
    serde_json::from_value(serde_json::json!({
        "show_id":effect["show_id"], "manifestation_id":effect["manifestation_id"],
        "mask_plan_id":effect["mask_plan_id"], "active_play_id":effect["mask_play"]["active_play_id"],
        "placement_id":effect["placement_id"], "face_id":effect["face_id"], "face_revision":effect["face_revision"],
    })).unwrap()
}

#[test]
fn whole_body_mask_executes_exact_admitted_slot_ack_and_typed_return() {
    let mut session = prepared_session();
    let reservations = session
        ._resource_admissions
        .as_ref()
        .unwrap()
        .admissions()
        .to_vec();
    let publication = request(&session, 1);
    let action = publication
        .face
        .actions
        .iter()
        .find(|action| {
            action.availability == conduit_face::FaceActionAvailability::Available
                && action.arguments.is_empty()
        })
        .unwrap()
        .clone();
    let prepared = session.body_face_prepare(publication).unwrap();
    assert_eq!(prepared["view"]["show_state"], "prepared");
    assert!(prepared["alternate"].is_null());
    let input = BrowserMaskInteraction {
        show_id: prepared["effect"]["show_id"].as_str().unwrap().into(),
        face_id: prepared["effect"]["face_id"].as_str().unwrap().into(),
        face_revision: 1,
        action_id: action.identity.clone(),
        target: action.target.clone(),
        arguments: vec![],
        sequence: 1,
    };
    assert!(session.body_face_interact(input.clone()).is_err());
    let shown = session.body_face_acknowledge(ack(&prepared)).unwrap();
    assert_eq!(shown["view"]["show_state"], "available");
    let returned = session.body_face_interact(input.clone()).unwrap();
    assert_eq!(
        returned["interaction"]["correlation"]["interaction"]["action_id"],
        action.identity
    );
    assert!(!returned["execution"]["remote_signs"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(session.body_face_interact(input).is_err());
    assert_eq!(
        session._resource_admissions.as_ref().unwrap().admissions(),
        reservations
    );
    let context = session.body_face_publication.as_ref().unwrap();
    let chain = &context
        .plan
        .mask_topologies
        .iter()
        .find(|t| t.face.plot.is_none())
        .unwrap()
        .chains[0];
    assert_eq!(
        prepared["effect"]["mask_plan_id"],
        chain.plan.plan_id.as_str()
    );
    assert_eq!(
        prepared["effect"]["placement_id"],
        chain.stage_placement_ids[0].as_str()
    );
}

#[test]
fn whole_body_mask_refuses_foreign_history_source_face_and_stale_revision() {
    let mut session = prepared_session();
    let good = request(&session, 1);
    for change in 0..8 {
        let mut bad = good.clone();
        match change {
            0 => bad.body_plan_id = "foreign/plan".into(),
            1 => bad.active_play_id = "foreign/play".into(),
            2 => {
                bad.body_evidence.friendly_name = "substituted".into();
                let bytes = serde_json::to_vec(&bad.body_evidence).unwrap();
                let attachment = BodyEvidenceAttachment::open_serialized(
                    &bytes,
                    BodyEvidenceEntrance::ExternalReader,
                )
                .unwrap();
                bad.face = conduit_body_make::project_body_workbench_face(
                    1,
                    &attachment,
                    &bad.reviewed_plots,
                )
                .unwrap();
            }
            3 => bad.reviewed_plots[0].checked_plot_id = "foreign/checked".into(),
            4 => bad.face = request(&session, 2).face,
            5 => {
                bad.reviewed_plots[0].label = "substituted label".into();
                let bytes = serde_json::to_vec(&bad.body_evidence).unwrap();
                let attachment = BodyEvidenceAttachment::open_serialized(
                    &bytes,
                    BodyEvidenceEntrance::ExternalReader,
                )
                .unwrap();
                bad.face = conduit_body_make::project_body_workbench_face(
                    1,
                    &attachment,
                    &bad.reviewed_plots,
                )
                .unwrap();
            }
            6 => {
                bad.revision = 2;
                bad.face = request(&session, 2).face;
            }
            _ => {
                let face = &bad.face;
                let mut subjects = face.subjects.clone();
                subjects[0].name = "forged owner wording".into();
                bad.face = Face::new_with_semantics(
                    face.revision,
                    face.basis.clone(),
                    subjects,
                    face.relationships.clone(),
                    face.properties.clone(),
                    face.text.clone(),
                    face.actions.clone(),
                    face.disclosures.clone(),
                )
                .unwrap();
            }
        }
        assert!(session.body_face_prepare(bad).is_err());
        assert!(session.body_face_observation().is_err());
    }
    let retained = session.body_face_prepare(good.clone()).unwrap();
    assert!(session.body_face_prepare(good).is_err());
    assert_eq!(session.body_face_observation().unwrap(), retained);
    for foreign_boot in [false, true] {
        let mut foreign = prepared_session();
        if foreign_boot {
            foreign.boot_id = "foreign/boot".into();
        } else {
            foreign.host_id = "foreign/host".into();
        }
        let proposed = request(&foreign, 1);
        assert!(foreign.body_face_prepare(proposed).is_err());
        assert!(foreign.body_face_observation().is_err());
    }
}

#[test]
fn whole_body_mask_replacement_reuses_reservation_and_stop_revokes_lease() {
    let mut session = prepared_session();
    let first = request(&session, 1);
    let first = session.body_face_prepare(first).unwrap();
    let reservations = session
        ._resource_admissions
        .as_ref()
        .unwrap()
        .admissions()
        .to_vec();
    let next = request(&session, 2);
    let next = session.body_face_prepare(next).unwrap();
    assert_ne!(first["effect"]["show_id"], next["effect"]["show_id"]);
    assert_eq!(
        first["effect"]["mask_plan_id"],
        next["effect"]["mask_plan_id"]
    );
    assert!(session.body_face_acknowledge(ack(&first)).is_err());
    assert_eq!(
        session._resource_admissions.as_ref().unwrap().admissions(),
        reservations
    );
    let context = session.body_face_publication.as_ref().unwrap();
    let lease = session
        .scheduler
        ._prepared_storage
        .publication_value_budget
        .as_ref()
        .unwrap()
        .admit(&context.plan.plan_id, &session.active_play_id, 0)
        .unwrap();
    session.cancel().unwrap();
    assert!(!lease.is_live());
}

#[test]
fn whole_body_mask_missing_selector_and_existing_parent_pressure_refuse() {
    let mut ordinary = super::super::body_start::prepare(
        super::super::body_start::tests::request_from_sources(&[SOURCE]),
    )
    .unwrap()
    .0;
    let other = prepared_session();
    assert!(ordinary.body_face_prepare(request(&other, 1)).is_err());
    let mut pressured = prepared_session();
    let proposed = request(&pressured, 1);
    let budget = pressured
        .scheduler
        ._prepared_storage
        .publication_value_budget
        .as_ref()
        .unwrap();
    let first = budget
        .admit(&proposed.body_plan_id, &proposed.active_play_id, 0)
        .unwrap();
    let second = budget
        .admit(&proposed.body_plan_id, &proposed.active_play_id, 0)
        .unwrap();
    assert!(pressured
        .body_face_prepare(proposed)
        .unwrap_err()
        .contains("lease capacity"));
    assert!(pressured.body_face_observation().is_err());
    drop((first, second));
}
