use super::*;

#[test]
fn sealed_session_routes_match_rich_routes_and_refuse_substitutions() {
    use conduit_body::{
        AuthenticatedHostObservation, BodyBiographyEvidence, BodyMembership, MembershipProofId,
        PartId, SealedBodyLifecycleSession,
    };
    let planned = plan_mask(
        "plot sealed-browser (\n >> face: Face\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.face\n output.show >> show\n input.interaction >> interaction\n}\n",
        "sealed-browser",
    );
    let fragment = &planned.plan.fragments[0];
    let host = &fragment.host_id;
    let boot = &fragment.boot_id;
    let resident = ResidentPlot::new(
        planned.mask.plot_identity.source_document_id.clone(),
        planned.mask.plot_identity.checked_plot_id.clone(),
    );
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(vec![resident.clone()]).unwrap(),
        1,
        "born".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Routes".into()).unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("routes-local-birth").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "admitted".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: fragment.offer_generation,
                proof_id: proof,
                sequence: 1,
            },
            "present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let mut session = SealedBodyLifecycleSession::open_admitted(evidence, host, boot).unwrap();
    let topology = BodyMaskTopology {
        face: BodyFaceSelector {
            plot: None,
            source_placement_id: None,
        },
        chains: vec![BodyMaskChainPlan {
            plan: planned.plan.clone(),
            stage_placement_ids: vec![fragment.placements[0].placement_id.clone()],
        }],
    };
    let proposal = session
        .propose_with_masks(
            vec![BodyPlotPlan {
                plot: resident,
                plan: planned.plan.clone(),
            }],
            vec![topology.clone()],
            host,
            boot,
        )
        .unwrap();
    let rich = BodyPlan::seal_with_masks(
        &proposal.wake,
        vec![BodyPlotPlan {
            plot: proposal.wake.workset.plots()[0].clone(),
            plan: planned.plan.clone(),
        }],
        vec![topology],
    )
    .unwrap();
    let routes = vec![route_for(&rich, &planned, "sealed")];
    let configurations = planned
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|gear| (gear.placement_id.clone(), gear.configuration.clone()))
        .collect::<Vec<_>>();
    let mut metadata = planned.plan.clone();
    for gear in metadata
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
    {
        gear.configuration.clear();
    }
    let verified = conduit_core::verify_plan_metadata_with_configurations(metadata, |gear| {
        Ok::<_, ()>(
            configurations
                .iter()
                .find(|(id, _)| id == &gear.placement_id)
                .unwrap()
                .1
                .clone(),
        )
    })
    .unwrap();
    assert_eq!(
        AdmittedMaskPlotRoutes::from_sealed_session(
            &session,
            &planned.mask,
            &verified,
            routes.clone()
        )
        .unwrap(),
        AdmittedMaskPlotRoutes::new(&rich, core::slice::from_ref(&planned), routes.clone())
            .unwrap(),
    );
    let mut wrong_role = planned.mask.clone();
    wrong_role.show_output.gear_port_id = "wrong-port".into();
    assert_eq!(
        AdmittedMaskPlotRoutes::from_sealed_session(
            &session,
            &wrong_role,
            &verified,
            routes.clone()
        ),
        Err(MaskRouteAdmissionError::InvalidPlan)
    );
    let mut bad_routes = routes.clone();
    bad_routes[0].plan_id = "plan/claimed".into();
    assert_eq!(
        AdmittedMaskPlotRoutes::from_sealed_session(&session, &planned.mask, &verified, bad_routes),
        Err(MaskRouteAdmissionError::WrongPlan)
    );
    let mut bad_routes = routes.clone();
    bad_routes[0].placement_ids[1] = bad_routes[0].placement_ids[0].clone();
    assert_eq!(
        AdmittedMaskPlotRoutes::from_sealed_session(&session, &planned.mask, &verified, bad_routes),
        Err(MaskRouteAdmissionError::MissingPlacement)
    );
    session.lull(host, boot, None).unwrap();
    assert_eq!(
        AdmittedMaskPlotRoutes::from_sealed_session(&session, &planned.mask, &verified, routes),
        Err(MaskRouteAdmissionError::InvalidPlan)
    );
}
