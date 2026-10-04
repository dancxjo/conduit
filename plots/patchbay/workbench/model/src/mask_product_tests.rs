use conduit_body::{
    BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology, BodyPlan, BodyPlotPlan, ResidentPlot,
};
use conduit_core::{
    bind_active_play, kind_id, port_id, ArtifactId, Back, BackOfferBuilder, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, Kind, KindIdentity, OfferGeneration, PortDescriptor, PortDirection,
    PortTemporal, SignId, PROTOCOL_VERSION,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    install_mask_plot_value_aliases, AdmittedMaskPlotRoutes, BodyMaskWardrobe,
    ManifestationLifecycle, MaskPlanningDisposition, MaskPlot, MaskShow, MaskShowDisposition,
    MaskWardrobe, MaskWardrobeLifetime, PlannedMaskPlot, Presentation, PresentationBasis,
    PresentationRole, PresentationSubject, PresentationText, SealedMaskPlotRoute,
    FACE_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND,
};

use conduit_presentation::{MaskWardrobeAction, MaskWardrobeControl};
use patchbay_application::project_mask_inspection;

struct Fixture {
    body: conduit_body::Body,
    wake: conduit_body::Wake,
    mask: MaskPlot,
    planned: PlannedMaskPlot,
    body_plan: BodyPlan,
    routes: AdmittedMaskPlotRoutes,
}

fn port(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

fn fixture(available: bool) -> Fixture {
    let definition = Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("presentation/test-mask"),
        kind_contract_revision: KindIdentity::from("conduit.test/presentation-mask@1"),
        inputs: vec![port(
            "presentation",
            PRESENTATION_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        outputs: vec![
            port(
                "interaction",
                FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            ),
        ],
        configuration: vec![],
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 2,
            max_queue_bytes: 64 * 1024,
        },
    };
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_mask_plot_value_aliases(&mut startup).unwrap();
    startup
        .insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .unwrap();
    profiles.insert_kind(definition.clone()).unwrap();
    let source = parse_syntax_document(
        "plot browser-mask (\n    >> face: Presentation\n    interaction: FaceInteraction...| >>\n    show: Show >>\n) {\n    mask: presentation/test-mask\n    face >> mask.presentation\n    mask.interaction >> interaction\n    mask.show >> show\n}\n",
    );
    let checked = check_syntax_document(&source, &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "browser-mask", &profiles).unwrap();
    let mask = MaskPlot::admit(&authoring).unwrap();
    let offer = BackOfferBuilder::new(
        definition,
        Back {
            capability_id: CapabilityId::from("cap/mask"),
            execution_profile_id: ExecutionProfileId::from("mask/test@1"),
            implementation_id: ImplementationId::from("implementation/mask"),
            artifact_id: ArtifactId::from("artifact/mask"),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/mask"),
        boot_id: BootId::from("boot/mask"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("mask/test@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let placements = conduit_planner::default_expanded_placements(
        &authoring.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let connection_bases = std::collections::BTreeMap::new();
    let line_candidates = std::collections::BTreeMap::new();
    let boundary_limits = authoring
        .front
        .inputs()
        .iter()
        .map(|port| (PortDirection::Input, port))
        .chain(
            authoring
                .front
                .outputs()
                .iter()
                .map(|port| (PortDirection::Output, port)),
        )
        .map(|(direction, port)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: port.port_id.clone(),
                    track: conduit_core::ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 64 * 1024,
                },
            )
        })
        .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &[host],
        &placements,
        &[],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 64 * 1024,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .unwrap();
    let planned = PlannedMaskPlot::admit(&mask, &plan).unwrap();
    let body = conduit_body::Body::born(
        mask.plot_identity.source_document_id.clone(),
        mask.plot_identity.checked_plot_id.clone(),
        1,
        SignId::from("sign/fixture-body-born"),
    )
    .unwrap();
    let (body, wake) = body
        .wake(1, SignId::from("sign/fixture-body-woke"))
        .unwrap();
    let resident = ResidentPlot::new(
        mask.plot_identity.source_document_id.clone(),
        mask.plot_identity.checked_plot_id.clone(),
    );
    let first_placement = plan.fragments[0].placements[0].placement_id.clone();
    let body_plan = BodyPlan::seal_with_masks(
        &wake,
        vec![BodyPlotPlan {
            plot: resident.clone(),
            plan: plan.clone(),
        }],
        vec![BodyMaskTopology {
            face: BodyFaceSelector {
                plot: Some(resident),
                source_placement_id: None,
            },
            chains: vec![BodyMaskChainPlan {
                plan: plan.clone(),
                stage_placement_ids: vec![first_placement],
            }],
        }],
    )
    .unwrap();
    let route = SealedMaskPlotRoute {
        route_id: "route/browser-mask".into(),
        mask_plot: mask.plot_identity.clone(),
        plan_id: body_plan.plan_id.clone(),
        child_mask_plan_id: None,
        owner_route_seal_id: None,
        placement_ids: plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| placement.placement_id.clone())
            .collect(),
        currently_available: available,
    };
    let routes =
        AdmittedMaskPlotRoutes::new(&body_plan, core::slice::from_ref(&planned), vec![route])
            .unwrap();
    Fixture {
        body,
        wake,
        mask,
        planned,
        body_plan,
        routes,
    }
}

#[test]
fn wardrobe_control_and_inspection_use_one_real_plot_plan() {
    let fixture = fixture(true);
    let body = fixture.body;
    let wake = fixture.wake;
    let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![], vec![]).unwrap();
    let scoped = BodyMaskWardrobe::new(body.body_id.clone(), None, wardrobe).unwrap();
    let mut control = MaskWardrobeControl::new(
        &body.body_id,
        scoped,
        &fixture.body_plan,
        &fixture.routes,
        None,
    )
    .unwrap();
    let evidence = control
        .apply(
            0,
            MaskWardrobeAction::Wear(fixture.mask.plot_identity.clone()),
            &fixture.routes,
        )
        .unwrap();
    let MaskShowDisposition::SelectSealed { selected, .. } = &evidence.reconciliation.show else {
        panic!("wearing the admitted Mask Plot must select its sealed route");
    };
    assert_eq!(selected.plan_id, fixture.body_plan.plan_id);

    let presentation = Presentation::new(
        1,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(fixture.mask.plot_identity.source_document_id.clone()),
            checked_plot_id: Some(fixture.mask.plot_identity.checked_plot_id.clone()),
            expanded_plot_id: Some(fixture.mask.plot_identity.expanded_plot_id.clone()),
            plan_id: Some(fixture.planned.plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: vec![SignId::from("sign/presentation")],
        },
        vec![PresentationSubject {
            identity: "mask/plot".into(),
            role: PresentationRole::Plot,
            name: "Browser Mask Plot".into(),
        }],
        vec![],
        vec![],
        vec![PresentationText {
            subject: "mask/plot".into(),
            text: "One ordinary Plot realizes this Presentation".into(),
        }],
    )
    .unwrap();
    let placement = fixture.planned.show_placement();
    let active_play = bind_active_play(
        &fixture.planned.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        1,
    );
    let show = MaskShow::prepared(
        &fixture.planned,
        &presentation,
        active_play,
        "mask/plot".into(),
        "browser/document".into(),
        SignId::from("sign/show-prepared"),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        SignId::from("sign/show-available"),
    )
    .unwrap();
    let projection = project_mask_inspection(
        &evidence.resulting_wardrobe,
        core::slice::from_ref(&fixture.mask.plot_identity),
        core::slice::from_ref(&fixture.planned),
        &fixture.routes,
        &evidence.reconciliation,
        Some(&show),
    )
    .unwrap();
    let projected = projection.current_show.unwrap();
    assert_eq!(projected.plan_id, fixture.body_plan.plan_id);
    assert_eq!(projected.mask_plan_id, fixture.planned.plan.plan_id);
    assert_eq!(projected.show_id, show.show_id.as_str());
    assert_eq!(
        projected.show_occurrence_id,
        show.show.manifestation_id.as_str()
    );
}

#[test]
fn unavailable_real_plan_route_truthfully_requests_replacement() {
    let fixture = fixture(false);
    let body_id = fixture.body.body_id;
    let wardrobe = MaskWardrobe::new(
        MaskWardrobeLifetime::Body,
        vec![fixture.mask.plot_identity],
        vec![],
    )
    .unwrap();
    let scoped = BodyMaskWardrobe::new(body_id.clone(), None, wardrobe).unwrap();
    let control =
        MaskWardrobeControl::new(&body_id, scoped, &fixture.body_plan, &fixture.routes, None)
            .unwrap();

    assert!(control.selected.is_none());
    let reconciliation = control
        .scoped_wardrobe
        .wardrobe
        .reconcile(&control.active_plan_id, fixture.routes.routes(), None)
        .unwrap();
    assert_eq!(
        reconciliation.planning,
        MaskPlanningDisposition::ReplacementRequired
    );
    assert!(matches!(
        reconciliation.show,
        MaskShowDisposition::NoCurrentShow { prior: None }
    ));
}
