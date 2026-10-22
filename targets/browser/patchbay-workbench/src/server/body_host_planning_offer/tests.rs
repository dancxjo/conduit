use super::*;
use conduit_body::CapabilitySummary;
use conduit_core::{CapabilityId, HostProfileId, ImplementationId, SignId};

fn projections(server: &PatchbayHtmlServer) -> (HostOfferProjection, HostOfferProjection) {
    let current = server
        .body_workload
        .as_ref()
        .unwrap()
        .evidence()
        .membership
        .parts
        .iter()
        .find_map(|part| part.current.as_ref())
        .unwrap();
    let mut host = conduit_std_host::StdHost::new().advertisement().clone();
    host.capabilities
        .push(conduit_std_host::hosted_keyboard_offer(
            "proof/body-keyboard",
            "proof/body-keyboard@1",
        ));
    host.resources.push(conduit_core::resource_offer(
        "proof/body-input",
        conduit_core::INPUT_RESOURCE_CLASS,
        1,
    ));
    let workset = &server
        .body_workload
        .as_ref()
        .unwrap()
        .evidence()
        .body
        .workset;
    let plots = planning_plots(workset, &server.body_planning_plots).unwrap();
    let requirements = conduit_body_make::body_planning_requirements(workset, &plots).unwrap();
    if server.selected_browser_mask {
        host.capabilities.push(conduit_browser_mask_offer::offer());
        // The fixture Host already offers one shared Face pool. A second
        // unowned pool of the same class would be ambiguous provenance.
        assert_eq!(
            host.resources
                .iter()
                .filter(|pool| pool.class_id.as_str() == conduit_core::FACE_RESOURCE_CLASS)
                .count(),
            1
        );
    }
    let mut kinds = requirements.kind_ids.clone();
    if server.selected_browser_mask {
        kinds.push(conduit_core::kind_id("face/browser-dom-mask"));
    }
    let mut offers = kinds
        .iter()
        .map(|kind| {
            host.capabilities
                .iter()
                .find(|offer| &offer.kind_id == kind)
                .unwrap()
                .clone()
        })
        .collect::<Vec<_>>();
    offers.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    offers.dedup_by(|left, right| left.capability_id == right.capability_id);
    let required_classes = offers
        .iter()
        .flat_map(|offer| {
            offer
                .resource_requirements
                .iter()
                .map(|item| &item.class_id)
        })
        .collect::<Vec<_>>();
    let mut resources = host
        .resources
        .iter()
        .filter(|offer| required_classes.contains(&&offer.class_id))
        .cloned()
        .collect::<Vec<_>>();
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let summary = HostOfferProjection {
        stage: OfferDisclosureStage::AdmittedMembership,
        protocol_version: PROTOCOL_VERSION,
        host_id: current.host_id.clone(),
        boot_id: current.boot_id.clone(),
        offer_generation: current.offer_generation,
        observation_sign_id: SignId::from("sign/browser/offer-summary"),
        freshness_sequence: 1,
        proof_class: RemoteProofClass::SelfReported,
        profile: Some(HostProfileId::from("browser-standard")),
        capability_summary: offers
            .iter()
            .map(|offer| CapabilitySummary {
                capability_id: offer.capability_id.clone(),
                implementation_id: offer.implementation.implementation_id.clone(),
            })
            .collect(),
        capabilities: Vec::new(),
        resources: Vec::new(),
    };
    let planning = HostOfferProjection {
        stage: OfferDisclosureStage::Planning,
        observation_sign_id: SignId::from("sign/browser/planning-detail"),
        freshness_sequence: 2,
        capability_summary: Vec::new(),
        capabilities: offers,
        resources,
        ..summary.clone()
    };
    (summary, planning)
}

#[test]
fn selected_browser_mask_is_exact_current_host_and_missing_offer_refuses_atomically() {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot)
        .unwrap()
        .with_body_planning_plots(crate::body_workbench_fixture_plots().unwrap())
        .unwrap()
        .with_selected_browser_mask()
        .unwrap();
    let (summary, offer) = projections(&server);
    server
        .apply_body_host_offer_evidence(&serde_json::to_vec(&summary).unwrap())
        .unwrap();
    let before = server.encoded_snapshot.clone();
    let mut missing = offer.clone();
    missing
        .capabilities
        .retain(|capability| capability.kind_id.as_str() != "face/browser-dom-mask");
    assert!(server
        .apply_body_host_planning_offer(&serde_json::to_vec(&missing).unwrap())
        .is_err());
    assert_eq!(server.encoded_snapshot, before);
    for foreign in [0, 1, 2] {
        let mut wrong = offer.clone();
        match foreign {
            0 => wrong.host_id = "host/foreign".into(),
            1 => wrong.boot_id = "boot/foreign".into(),
            _ => wrong.offer_generation.0 += 1,
        }
        assert!(server
            .apply_body_host_planning_offer(&serde_json::to_vec(&wrong).unwrap())
            .is_err());
        assert_eq!(server.encoded_snapshot, before);
    }
    server
        .apply_body_host_planning_offer(&serde_json::to_vec(&offer).unwrap())
        .unwrap();
    let planning = server.body_planning.as_ref().unwrap();
    let topology = &planning.current_plan().mask_topologies[0];
    assert!(topology.face.plot.is_none());
    assert_eq!(topology.chains.len(), 1);
    let fragment = &topology.chains[0].plan.fragments[0];
    assert_eq!(fragment.host_id, offer.host_id);
    assert_eq!(fragment.boot_id, offer.boot_id);
    assert_eq!(fragment.offer_generation, offer.offer_generation);
    assert!(server
        .mask_control
        .as_ref()
        .unwrap()
        .selected_graphical_pending());
    assert!(server.body_face_show.is_none());
    let proposal: serde_json::Value =
        serde_json::from_slice(&server.body_execution_proposal().unwrap()).unwrap();
    assert_eq!(
        proposal["owner_face_revision"],
        server
            .snapshot
            .body_workbench
            .as_ref()
            .unwrap()
            .evidence_revision
            .to_string()
    );
}

#[test]
fn typed_selected_show_lull_waits_for_matching_terminal_and_consumes_once() {
    use serde_json::json;
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot)
        .unwrap()
        .with_body_planning_plots(crate::body_workbench_fixture_plots().unwrap())
        .unwrap()
        .with_selected_browser_mask()
        .unwrap();
    let (summary, offer) = projections(&server);
    server
        .apply_body_host_offer_evidence(&serde_json::to_vec(&summary).unwrap())
        .unwrap();
    server
        .apply_body_host_planning_offer(&serde_json::to_vec(&offer).unwrap())
        .unwrap();
    let baseline = server.snapshot.face.revision;
    let plan = server
        .body_planning
        .as_ref()
        .unwrap()
        .current_plan()
        .clone();
    let request = |action| {
        serde_json::to_vec(&json!({"schema":"conduit.body/execution-request@1","action":action}))
            .unwrap()
    };
    server.apply_body_execution(&request(json!({"kind":"Claim","plan_id":plan.plan_id,"host_id":offer.host_id,"boot_id":offer.boot_id}))).unwrap();
    let claim = server
        .snapshot
        .body_planning
        .as_ref()
        .unwrap()
        .execution_claims[0]
        .clone();
    let sign = |sequence| {
        conduit_core::bind_sign(
            &claim.host_id,
            &claim.boot_id,
            Some(&claim.play.active_play_id),
            sequence,
        )
        .sign_id
    };
    let planning = server.body_planning.as_ref().unwrap();
    let wake = planning
        .wake()
        .body_plan_ready(&plan, sign(0))
        .unwrap()
        .body_play_started(&plan, &claim.play, sign(1))
        .unwrap();
    server
        .apply_body_execution(&request(
            json!({"kind":"Started","play":claim.play,"wake_at_start":wake}),
        ))
        .unwrap();
    assert_eq!(server.snapshot.face.revision, baseline + 1);
    assert!(server.body_face_publication().is_ok());
    let planned = conduit_browser_mask_offer::planned_mask(
        &advertisement(&offer).unwrap(),
        conduit_browser_mask_offer::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    let placement = planned.show_placement();
    let mask_play = conduit_core::bind_active_play(
        &planned.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        1,
    );
    let prepared = conduit_face::MaskShow::prepared(
        &planned,
        &server.snapshot.face,
        mask_play,
        server
            .snapshot
            .face
            .subjects
            .first()
            .unwrap()
            .identity
            .clone(),
        "browser/document".into(),
        "sign/prepared".into(),
    )
    .unwrap();
    let show = prepared
        .transition(
            conduit_face::ManifestationLifecycle::Available,
            "sign/available".into(),
        )
        .unwrap();
    let observation = |show: &conduit_face::MaskShow,
                       interaction: serde_json::Value,
                       face: &conduit_face::Face| {
        serde_json::to_vec(&json!({
        "schema":"conduit.browser/body-face-observation@1","body_plan_id":plan.plan_id,"active_play_id":claim.play.active_play_id,
        "face":face,"mask_show":show,"interaction":interaction,
    })).unwrap()
    };
    let before = server.encoded_snapshot.clone();
    assert!(server
        .acknowledge_body_face(&observation(
            &prepared,
            serde_json::Value::Null,
            &server.snapshot.face
        ))
        .is_err());
    assert_eq!(before, server.encoded_snapshot);
    server
        .acknowledge_body_face(&observation(
            &show,
            serde_json::Value::Null,
            &server.snapshot.face,
        ))
        .unwrap();
    assert!(server
        .acknowledge_body_face(&observation(
            &show,
            serde_json::Value::Null,
            &server.snapshot.face
        ))
        .is_err());
    let action = server
        .snapshot
        .face
        .actions
        .iter()
        .find(|action| action.intent == "conduit.intent/lull@1")
        .unwrap();
    let interaction = conduit_face::FaceInteraction::new(
        &server.snapshot.face,
        &show,
        &action.identity,
        &action.target,
        vec![],
        1,
    )
    .unwrap();
    let returned = observation(
        &show,
        json!({"correlation":show.correlate_interaction(interaction).unwrap()}),
        &server.snapshot.face,
    );
    let before = server.encoded_snapshot.clone();
    let pending: serde_json::Value =
        serde_json::from_slice(&server.interact_body_face(&returned).unwrap()).unwrap();
    assert_eq!(pending["disposition"], "awaiting-terminal");
    assert_ne!(before, server.encoded_snapshot);
    assert!(server.snapshot.mask_topology.is_none());
    assert_eq!(
        server.body_planning.as_ref().unwrap().wake().lifecycle,
        conduit_body::WakeLifecycle::Playing
    );
    let before = server.encoded_snapshot.clone();
    assert!(server.body_face_show.is_none());
    assert!(server.interact_body_face(&returned).is_err());
    assert!(server.body_face_publication().is_err());
    assert!(server
        .apply_body_execution(&request(json!({"kind":"Lull","wake_id":wake.wake_id})))
        .is_err());
    assert_eq!(before, server.encoded_snapshot);
    let mut foreign = claim.play.clone();
    foreign.active_play_id = "play/foreign".into();
    assert!(server.apply_body_execution(&request(json!({"kind":"Terminal","play":foreign,"disposition":"cancelled","terminal_sign_id":sign(2)}))).is_err());
    assert!(server.pending_face_lull.is_some());
    assert_eq!(before, server.encoded_snapshot);
    let terminal = request(
        json!({"kind":"Terminal","play":claim.play,"disposition":"cancelled","terminal_sign_id":sign(2)}),
    );
    server.apply_body_execution(&terminal).unwrap();
    assert!(server.pending_face_lull.is_none());
    assert_eq!(
        server.body_planning.as_ref().unwrap().wake().lifecycle,
        conduit_body::WakeLifecycle::Lulled
    );
    assert!(server.body_face_show.is_none());
    assert!(server.body_face_publication().is_err());
    assert!(server.apply_body_execution(&terminal).is_err());
}

#[test]
fn explicit_policy_admission_starts_an_ordinary_body_plan() {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let body_id = snapshot.body_workbench.as_ref().unwrap().body_id.clone();
    let plots = crate::body_workbench_fixture_plots().unwrap();
    let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot)
        .unwrap()
        .with_body_planning_plots(plots)
        .unwrap();
    let (summary, planning) = projections(&server);
    eprintln!(
        "measured Body planning offer: {} bytes",
        serde_json::to_vec(&planning).unwrap().len()
    );
    server
        .apply_body_host_offer_evidence(&serde_json::to_vec(&summary).unwrap())
        .unwrap();

    let admitted: crate::RendererSnapshot = serde_json::from_slice(
        &server
            .apply_body_host_planning_offer(&serde_json::to_vec(&planning).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(admitted.body_host_planning_offer.as_ref(), Some(&planning));
    assert_eq!(admitted.body_workbench.as_ref().unwrap().body_id, body_id);
    assert!(admitted.body_planning.is_some());
    let retained: conduit_body::BodyBiographyEvidence =
        serde_json::from_slice(&server.current_body_evidence().unwrap()).unwrap();
    retained.validate().unwrap();
    let planning_session = server.body_planning.as_ref().unwrap();
    assert_eq!(&retained.body, planning_session.body());
    assert_eq!(retained.wakes, vec![planning_session.wake().clone()]);
    assert_eq!(
        admitted.body_planning.as_ref().unwrap().lifecycle,
        conduit_body::WakeLifecycle::AwaitingPlan
    );
    assert!(server
        .body_planning
        .as_ref()
        .unwrap()
        .wake()
        .plans
        .is_empty());
    assert_eq!(
        admitted.body_planning.as_ref().unwrap().body_id.as_str(),
        body_id
    );
    assert_eq!(
        admitted.interaction.last_disposition.as_deref(),
        Some("Succeeded(PlanningInputAdmitted;BodyReplanned)")
    );
}

#[test]
fn stale_boot_and_detail_absent_from_summary_refuse_atomically() {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let plots = crate::body_workbench_fixture_plots().unwrap();
    let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot)
        .unwrap()
        .with_body_planning_plots(plots)
        .unwrap();
    let (summary, mut planning) = projections(&server);
    server
        .apply_body_host_offer_evidence(&serde_json::to_vec(&summary).unwrap())
        .unwrap();
    let prior = server.encoded_snapshot.clone();

    planning.boot_id = conduit_core::BootId::from("stale/browser-boot");
    assert!(server
        .apply_body_host_planning_offer(&serde_json::to_vec(&planning).unwrap())
        .is_err());
    assert_eq!(server.encoded_snapshot, prior);

    let (_, mut planning) = projections(&server);
    planning.capabilities[0].capability_id = CapabilityId::from("unadvertised-capability");
    planning.capabilities[0].implementation.implementation_id =
        ImplementationId::from("unadvertised-implementation");
    assert!(server
        .apply_body_host_planning_offer(&serde_json::to_vec(&planning).unwrap())
        .is_err());
    assert_eq!(server.encoded_snapshot, prior);
}
