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
