use super::*;
use conduit_body::{BodyBiographyEvidence, BodyLifecycleSession, BodyMembership};
use conduit_presentation::{
    BodyMaskWardrobe, CurrentOwnerPresentationRoute, LocalOwnerMaskRouteSeal, MaskShowDisposition,
    MaskWardrobe, MaskWardrobeControl, MaskWardrobeLifetime, OwnerPresentationPlan,
    OwnerPresentationPlanError,
};

#[test]
fn owner_presentation_ensemble_preserves_child_plans_and_reconciles_sealed_alternatives() {
    let browser_source = "plot browser (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n";
    let alternate_source = browser_source.replace("plot browser", "plot alternate-browser");
    let (browser, browser_host) = plan_mask_with_host(browser_source, "browser");
    let (alternate, alternate_host) = plan_mask_with_host(&alternate_source, "alternate-browser");
    let body = Body::born(
        browser.mask.plot_identity.source_document_id.clone(),
        browser.mask.plot_identity.checked_plot_id.clone(),
        1,
        SignId::from("sign/ensemble-born"),
    )
    .unwrap();
    let body_id = body.body_id.clone();
    let membership = BodyMembership::new(body_id.clone()).unwrap();
    let evidence = BodyBiographyEvidence::born(body, membership, "Ensemble".into()).unwrap();
    let session = BodyLifecycleSession::open(evidence).unwrap();
    let face = Presentation::new(
        1,
        PresentationBasis {
            body_id: Some(body_id.clone()),
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "ensemble/plot".into(),
            role: PresentationRole::Plot,
            name: "Ensemble".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let browser_seal =
        LocalOwnerMaskRouteSeal::seal_lulled(&session, &face, &browser_host, &browser).unwrap();
    let alternate_seal =
        LocalOwnerMaskRouteSeal::seal_lulled(&session, &face, &alternate_host, &alternate).unwrap();
    let witnesses = [
        CurrentOwnerPresentationRoute::Local {
            seal: &browser_seal,
            owner_offer: &browser_host,
        },
        CurrentOwnerPresentationRoute::Local {
            seal: &alternate_seal,
            owner_offer: &alternate_host,
        },
    ];
    let ensemble = OwnerPresentationPlan::seal_current(&session, &face, &witnesses).unwrap();
    ensemble.verify_seal().unwrap();
    let routes = ensemble
        .admit_current_routes(&session, &face, &witnesses)
        .unwrap();
    assert_eq!(routes.plan_id(), &ensemble.plan_id);
    assert!(routes
        .routes()
        .iter()
        .any(
            |route| route.child_mask_plan_id.as_ref() == Some(&browser.plan.plan_id)
                && route.owner_route_seal_id.as_ref() == Some(&browser_seal.route_plan_id)
        ));
    assert!(routes
        .routes()
        .iter()
        .any(
            |route| route.child_mask_plan_id.as_ref() == Some(&alternate.plan.plan_id)
                && route.owner_route_seal_id.as_ref() == Some(&alternate_seal.route_plan_id)
        ));
    let wardrobe = MaskWardrobe::new(
        MaskWardrobeLifetime::Body,
        vec![
            browser.mask.plot_identity.clone(),
            alternate.mask.plot_identity.clone(),
        ],
        vec![
            browser.mask.plot_identity.clone(),
            alternate.mask.plot_identity.clone(),
        ],
    )
    .unwrap();
    let scoped = BodyMaskWardrobe::new(body_id, None, wardrobe).unwrap();
    let mut control = MaskWardrobeControl::new_from_admitted_routes(scoped, &routes, None).unwrap();
    assert!(
        matches!(control.selected.as_ref(), Some(selected) if selected.mask_plot == browser.mask.plot_identity)
    );
    let remaining = ensemble
        .admit_current_routes(&session, &face, &witnesses[1..])
        .unwrap();
    let fallback = control.reconcile_routes(&remaining).unwrap();
    assert!(
        matches!(fallback.show, MaskShowDisposition::SelectSealed { selected, .. }
        if selected.mask_plot == alternate.mask.plot_identity)
    );
    let absent = ensemble.admit_current_routes(&session, &face, &[]).unwrap();
    let replacement = control.reconcile_routes(&absent).unwrap();
    assert!(matches!(
        replacement.show,
        MaskShowDisposition::NoCurrentShow { .. }
    ));
    let mut forged = ensemble.clone();
    forged.plan_id = PlanId::from("plan/owner-presentation/forged");
    assert_eq!(
        forged.admit_current_routes(&session, &face, &witnesses),
        Err(OwnerPresentationPlanError::InvalidIdentity)
    );
    let mut forged_child = ensemble.clone();
    forged_child.routes[0] = forged_child.routes[1].clone();
    assert!(forged_child.verify_seal().is_err());
    let mut wrong_child_plan = ensemble.clone();
    if let conduit_presentation::OwnerPresentationChildRoute::Local { seal } =
        &mut wrong_child_plan.routes[0]
    {
        seal.planned_mask.plan.plan_id = PlanId::from("plan/child/forged");
    }
    assert!(wrong_child_plan.verify_seal().is_err());
    assert_eq!(
        OwnerPresentationPlan::seal_current(&session, &face, &[witnesses[0], witnesses[0]]),
        Err(OwnerPresentationPlanError::DuplicateRoute)
    );
    let mut too_many = Vec::new();
    too_many.resize(
        conduit_presentation::MAX_OWNER_PRESENTATION_ROUTES + 1,
        witnesses[0],
    );
    assert_eq!(
        OwnerPresentationPlan::seal_current(&session, &face, &too_many),
        Err(OwnerPresentationPlanError::EmptyOrTooManyRoutes)
    );
    let mut stale_host = browser_host.clone();
    stale_host.offer_generation.0 += 1;
    assert!(ensemble
        .admit_current_routes(
            &session,
            &face,
            &[CurrentOwnerPresentationRoute::Local {
                seal: &browser_seal,
                owner_offer: &stale_host
            },]
        )
        .is_err());
    let mut stale_face = face.clone();
    stale_face.revision += 1;
    assert_eq!(
        ensemble.admit_current_routes(&session, &stale_face, &witnesses),
        Err(OwnerPresentationPlanError::StaleBodyOrFace)
    );
}

#[test]
fn mixed_local_remote_ensemble_requires_current_part_and_directional_lines() {
    use conduit_body::{AuthenticatedHostObservation, MembershipProofId, PartId};
    use conduit_core::{
        process_owned_line_offer_with_limits, AuthorityGrantId, BaseImplementationId,
        CredentialReferenceId, LineAvailability, LineScope, LineSecurity, LinkAuthorityReference,
        LinkCredentialReference, LinkLimits,
    };
    use conduit_presentation::{RemoteOwnerMaskRouteError, RemoteOwnerMaskRouteSeal};

    let local_source = "plot terminal (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n";
    let remote_source = local_source.replace("plot terminal", "plot browser");
    let (terminal, owner_offer) = plan_mask_with_host_on(local_source, "terminal", "host/owner");
    let (browser, browser_offer) =
        plan_mask_with_host_on(&remote_source, "browser", "host/browser");
    let body = Body::born(
        terminal.mask.plot_identity.source_document_id.clone(),
        terminal.mask.plot_identity.checked_plot_id.clone(),
        1,
        SignId::from("sign/mixed-born"),
    )
    .unwrap();
    let body_id = body.body_id.clone();
    let mut membership = BodyMembership::new(body_id.clone()).unwrap();
    let part = PartId::bind(&body_id, browser_offer.host_id.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("mixed-browser").unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body, membership.clone(), "Mixed".into()).unwrap();
    let admitted = membership
        .admit(
            &body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from("sign/mixed-admitted"),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: browser_offer.host_id.clone(),
                boot_id: browser_offer.boot_id.clone(),
                offer_generation: browser_offer.offer_generation,
                proof_id: proof,
                sequence: 1,
            },
            SignId::from("sign/mixed-present"),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let session = BodyLifecycleSession::open(evidence).unwrap();
    let face = Presentation::new(
        3,
        PresentationBasis {
            body_id: Some(body_id.clone()),
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "mixed/plot".into(),
            role: PresentationRole::Plot,
            name: "Mixed routes".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 64 * 1024,
        maximum_buffered_bytes: 128 * 1024,
        maximum_frame_bytes: 64 * 1024,
    };
    // Explicit contract fixtures: these do not prove a live browser carrier.
    let line = |direction: &str, source: &HostAdvertisement, sink: &HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            &format!("line/mixed/{direction}"),
            &format!("binding/mixed/{direction}"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "base-instance/mixed",
            source,
            sink,
            limits,
        );
        line.binding.credential =
            LinkCredentialReference::Opaque(CredentialReferenceId::from("credential/mixed"));
        line.binding.authority =
            LinkAuthorityReference::Grant(AuthorityGrantId::from("grant/mixed"));
        line.contract.scope = LineScope::RoutedNetwork;
        line.contract.security = LineSecurity::PlaintextNetwork;
        line
    };
    let face_line = line("face", &owner_offer, &browser_offer);
    let return_line = line("return", &browser_offer, &owner_offer);
    let local =
        LocalOwnerMaskRouteSeal::seal_lulled(&session, &face, &owner_offer, &terminal).unwrap();
    let remote = RemoteOwnerMaskRouteSeal::seal_lulled(
        &session,
        &face,
        &owner_offer,
        &browser_offer,
        &browser,
        &face_line,
        &return_line,
    )
    .unwrap();
    let witnesses = [
        CurrentOwnerPresentationRoute::Local {
            seal: &local,
            owner_offer: &owner_offer,
        },
        CurrentOwnerPresentationRoute::Remote {
            seal: &remote,
            owner_offer: &owner_offer,
            mask_host_offer: &browser_offer,
            face_line: &face_line,
            return_line: &return_line,
        },
    ];
    let ensemble = OwnerPresentationPlan::seal_current(&session, &face, &witnesses).unwrap();
    let both = ensemble
        .admit_current_routes(&session, &face, &witnesses)
        .unwrap();
    assert_eq!(both.routes().len(), 2);
    assert!(both
        .routes()
        .iter()
        .any(
            |route| route.child_mask_plan_id.as_ref() == Some(&browser.plan.plan_id)
                && route.owner_route_seal_id.as_ref() == Some(&remote.route_plan_id)
                && route.currently_available
        ));
    let wardrobe = MaskWardrobe::new(
        MaskWardrobeLifetime::Body,
        vec![
            terminal.mask.plot_identity.clone(),
            browser.mask.plot_identity.clone(),
        ],
        vec![
            browser.mask.plot_identity.clone(),
            terminal.mask.plot_identity.clone(),
        ],
    )
    .unwrap();
    let mut control = MaskWardrobeControl::new_from_admitted_routes(
        BodyMaskWardrobe::new(body_id, None, wardrobe).unwrap(),
        &both,
        None,
    )
    .unwrap();
    assert!(
        matches!(control.selected.as_ref(), Some(selected) if selected.mask_plot == browser.mask.plot_identity)
    );
    let local_only = ensemble
        .admit_current_routes(&session, &face, &witnesses[..1])
        .unwrap();
    assert!(local_only
        .routes()
        .iter()
        .any(
            |route| route.owner_route_seal_id.as_ref() == Some(&remote.route_plan_id)
                && !route.currently_available
        ));
    assert!(
        matches!(control.reconcile_routes(&local_only).unwrap().show,
        MaskShowDisposition::SelectSealed { selected, .. }
        if selected.mask_plot == terminal.mask.plot_identity)
    );
    let mut lost_return = return_line.clone();
    lost_return.availability.availability = LineAvailability::Unavailable;
    assert_eq!(
        ensemble.admit_current_routes(
            &session,
            &face,
            &[
                witnesses[0],
                CurrentOwnerPresentationRoute::Remote {
                    seal: &remote,
                    owner_offer: &owner_offer,
                    mask_host_offer: &browser_offer,
                    face_line: &face_line,
                    return_line: &lost_return,
                },
            ]
        ),
        Err(OwnerPresentationPlanError::Remote(
            RemoteOwnerMaskRouteError::LineUnavailable
        ))
    );
    let mut wrong_boot = face_line.clone();
    wrong_boot.binding.sink.boot_id = BootId::from("boot/browser-stale");
    assert_eq!(
        ensemble.admit_current_routes(
            &session,
            &face,
            &[
                witnesses[0],
                CurrentOwnerPresentationRoute::Remote {
                    seal: &remote,
                    owner_offer: &owner_offer,
                    mask_host_offer: &browser_offer,
                    face_line: &wrong_boot,
                    return_line: &return_line,
                },
            ]
        ),
        Err(OwnerPresentationPlanError::Remote(
            RemoteOwnerMaskRouteError::StaleLine
        ))
    );
}
