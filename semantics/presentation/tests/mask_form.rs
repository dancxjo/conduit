#![cfg(feature = "form-catalog")]

use conduit_body::{
    Body, BodyFaceSelector, BodyFormPlan, BodyMaskChainPlan, BodyMaskTopology, BodyPlan,
    BodyWorkset, ResidentForm,
};
use conduit_core::{
    bind_active_play, kind_id, port_id, ArtifactId, Back, BackOfferBuilder, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, Kind, KindIdentity, OfferGeneration, PlanId, PortDescriptor, PortDirection,
    PortTemporal, SignId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    install_mask_form_value_aliases, AdmittedMaskFormRoutes, ManifestationLifecycle, MaskForm,
    MaskFormError, MaskRouteAdmissionError, MaskShow, PlannedMaskForm, Presentation,
    PresentationBasis, PresentationRole, PresentationSubject, PresentationText,
    SealedMaskFormRoute, FACE_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND,
};

fn port(
    name: &str,
    kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

fn kind(name: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(format!("conduit.test/{name}@1")),
        inputs,
        outputs,
        configuration: vec![],
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: 64 * 1024,
        },
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let definitions = vec![
        kind(
            "presentation/layout",
            vec![port(
                "presentation",
                PRESENTATION_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "scene",
                "graphics/scene@1",
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "presentation/composite",
            vec![port(
                "scene",
                "graphics/scene@1",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "frame",
                "graphics/frame@1",
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "display/show",
            vec![port(
                "frame",
                "graphics/frame@1",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "display/input",
            vec![],
            vec![port(
                "interaction",
                FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            )],
        ),
        kind(
            "web/dom",
            vec![port(
                "presentation",
                PRESENTATION_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "web/input",
            vec![],
            vec![port(
                "interaction",
                FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            )],
        ),
        kind(
            "presentation/aural",
            vec![port(
                "presentation",
                PRESENTATION_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "text",
                "text/text@1",
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "speech/synthesize",
            vec![port(
                "text",
                "text/text@1",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "audio",
                "audio/pcm@1",
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "audio/play",
            vec![port(
                "audio",
                "audio/pcm@1",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "audio/listen",
            vec![],
            vec![port(
                "interaction",
                FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            )],
        ),
    ];
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    for definition in definitions {
        startup
            .insert(KindSignature {
                kind: definition.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles.insert_kind(definition).unwrap();
    }
    install_mask_form_value_aliases(&mut startup).unwrap();
    startup
        .insert_value_kind_alias("Text", kind_id("text/text@1"))
        .unwrap();
    (startup, profiles)
}

fn admit(source: &str, name: &str) -> MaskForm {
    let (startup, profiles) = catalogs();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form_for_authoring(&checked, name, &profiles).unwrap();
    MaskForm::admit(&expanded).unwrap()
}

fn host_for(
    expanded: &conduit_form::ExpandedCanonicalForm,
    profiles: &ProfileCatalog,
) -> HostAdvertisement {
    let capabilities = expanded
        .gears
        .iter()
        .map(|gear| {
            BackOfferBuilder::new(
                profiles.canonical_kind(&gear.kind_id).unwrap().clone(),
                Back {
                    capability_id: CapabilityId::from(format!("cap/{}", gear.gear_id.as_str())),
                    execution_profile_id: ExecutionProfileId::from("mask/test@1"),
                    implementation_id: ImplementationId::from(format!(
                        "implementation/{}",
                        gear.gear_id.as_str()
                    )),
                    artifact_id: ArtifactId::from(format!("artifact/{}", gear.gear_id.as_str())),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build()
        })
        .collect();
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/mask-test"),
        boot_id: BootId::from("boot/mask-test"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("mask/test@1"),
        bases: vec![],
        resources: vec![],
        capabilities,
        planner_capabilities: vec![],
    }
}

fn body_plan_for(planned_masks: &[PlannedMaskForm]) -> BodyPlan {
    let residents = planned_masks
        .iter()
        .map(|planned| {
            ResidentForm::new(
                planned.mask.form_identity.source_document_id.clone(),
                planned.mask.form_identity.checked_form_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    let body = Body::born_with_forms(
        BodyWorkset::from_forms(residents.clone()).unwrap(),
        1,
        SignId::from("sign/mask-body-born"),
    )
    .unwrap();
    let wake = body.wake(1, SignId::from("sign/mask-body-woke")).unwrap().1;
    let forms = residents
        .into_iter()
        .zip(planned_masks)
        .map(|(form, planned)| BodyFormPlan {
            form,
            plan: planned.plan.clone(),
        })
        .collect();
    let topologies = planned_masks
        .iter()
        .map(|planned| {
            let first = planned
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .next()
                .unwrap()
                .placement_id
                .clone();
            BodyMaskTopology {
                face: BodyFaceSelector {
                    form: Some(ResidentForm::new(
                        planned.mask.form_identity.source_document_id.clone(),
                        planned.mask.form_identity.checked_form_id.clone(),
                    )),
                    source_placement_id: first.clone(),
                },
                chains: vec![BodyMaskChainPlan {
                    plan: planned.plan.clone(),
                    stage_placement_ids: vec![first],
                }],
            }
        })
        .collect();
    BodyPlan::seal_with_masks(&wake, forms, topologies).unwrap()
}

fn plan_mask(source: &str, name: &str) -> PlannedMaskForm {
    let (startup, profiles) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring = expand_canonical_form_for_authoring(&checked, name, &profiles).unwrap();
    let mask = MaskForm::admit(&authoring).unwrap();
    let host = host_for(&authoring.expanded, &profiles);
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
    PlannedMaskForm::admit(&mask, &plan).unwrap()
}

fn route_for(body_plan: &BodyPlan, planned: &PlannedMaskForm, name: &str) -> SealedMaskFormRoute {
    SealedMaskFormRoute {
        route_id: format!("route/{name}"),
        mask_form: planned.mask.form_identity.clone(),
        plan_id: body_plan.plan_id.clone(),
        placement_ids: planned
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| placement.placement_id.clone())
            .collect(),
        currently_available: true,
    }
}

#[test]
fn two_mask_forms_share_one_body_plan_identity_and_unsealed_masks_refuse() {
    let browser = plan_mask(
        "form browser (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n",
        "browser",
    );
    let spoken = plan_mask(
        "form alternate-browser (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n",
        "alternate-browser",
    );
    assert_ne!(browser.mask.form_identity, spoken.mask.form_identity);
    let body_plan = body_plan_for(&[browser.clone(), spoken.clone()]);
    let routes = vec![
        route_for(&body_plan, &browser, "browser"),
        route_for(&body_plan, &spoken, "spoken"),
    ];
    let admitted =
        AdmittedMaskFormRoutes::new(&body_plan, &[browser.clone(), spoken.clone()], routes)
            .unwrap();
    assert_eq!(admitted.plan_id(), &body_plan.plan_id);
    assert!(admitted
        .routes()
        .iter()
        .all(|route| route.plan_id == body_plan.plan_id));

    let mut forged_body_plan = body_plan.clone();
    forged_body_plan.plan_id = PlanId::from("body-plan/forged");
    assert_eq!(
        AdmittedMaskFormRoutes::new(
            &forged_body_plan,
            &[browser.clone(), spoken.clone()],
            admitted.routes().to_vec(),
        ),
        Err(MaskRouteAdmissionError::InvalidPlan)
    );

    let possible_but_unsealed = plan_mask(
        "form late-browser (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n",
        "late-browser",
    );
    assert_eq!(
        AdmittedMaskFormRoutes::new(
            &body_plan,
            core::slice::from_ref(&possible_but_unsealed),
            vec![route_for(
                &body_plan,
                &possible_but_unsealed,
                "late-browser"
            )],
        ),
        Err(MaskRouteAdmissionError::UnsealedMaskForm)
    );
}

#[test]
fn graphical_browser_and_spoken_masks_are_ordinary_forms_with_one_role_boundary() {
    let native = admit(
        "form native-graphical (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n layout: presentation/layout\n compose: presentation/composite\n output: display/show\n input: display/input\n face >> layout.presentation\n layout.scene >> compose.scene\n compose.frame >> output.frame\n output.show >> show\n input.interaction >> interaction\n}\n",
        "native-graphical",
    );
    let browser = admit(
        "form browser-graphical (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n",
        "browser-graphical",
    );
    let spoken = admit(
        "form spoken (\n voice-name: Text = \"calm\"\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n language: presentation/aural\n voice: speech/synthesize\n output: audio/play\n input: audio/listen\n face >> language.presentation\n language.text >> voice.text\n voice.audio >> output.audio\n output.show >> show\n input.interaction >> interaction\n}\n",
        "spoken",
    );

    assert_eq!(native.form_name, "native-graphical");
    assert_eq!(browser.form_name, "browser-graphical");
    assert_eq!(spoken.form_name, "spoken");
    assert_ne!(
        native.form_identity.checked_form_id,
        browser.form_identity.checked_form_id
    );
    assert_ne!(
        browser.form_identity.checked_form_id,
        spoken.form_identity.checked_form_id
    );
    for mask in [native, browser, spoken] {
        assert_eq!(mask.face_input.front_port_id.as_str(), "face");
        assert_eq!(
            mask.interaction_output.front_port_id.as_str(),
            "interaction"
        );
        assert_eq!(mask.show_output.front_port_id.as_str(), "show");
    }
}

#[test]
fn an_ordinary_form_without_the_mask_role_boundary_is_not_a_mask() {
    let (startup, profiles) = catalogs();
    let syntax = parse_syntax_document(
        "form tutorial (\n >> face: Presentation\n show: Show >>\n) {\n output: web/dom\n face >> output.presentation\n output.show >> show\n}\n",
    );
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form_for_authoring(&checked, "tutorial", &profiles).unwrap();
    assert!(MaskForm::admit(&expanded).is_err());
}

#[test]
fn the_old_presentation_named_fore_is_not_a_mask_alias() {
    let (startup, profiles) = catalogs();
    let syntax = parse_syntax_document(
        "form legacy-mask (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n presentation >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n",
    );
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form_for_authoring(&checked, "legacy-mask", &profiles).unwrap();
    assert_eq!(
        MaskForm::admit(&expanded),
        Err(MaskFormError::MissingFaceInput)
    );
}

#[test]
fn the_ordinary_planner_seals_the_mask_form_without_a_mask_planner() {
    let source = "form browser-graphical (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n";
    let (startup, profiles) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring =
        expand_canonical_form_for_authoring(&checked, "browser-graphical", &profiles).unwrap();
    let mask = MaskForm::admit(&authoring).unwrap();
    let host = host_for(&authoring.expanded, &profiles);
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
    let planned = PlannedMaskForm::admit(&mask, &plan).unwrap();
    let body_plan = body_plan_for(core::slice::from_ref(&planned));
    let route = SealedMaskFormRoute {
        route_id: "route/browser".into(),
        mask_form: mask.form_identity.clone(),
        plan_id: body_plan.plan_id.clone(),
        placement_ids: plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| placement.placement_id.clone())
            .collect(),
        currently_available: true,
    };
    assert!(AdmittedMaskFormRoutes::new(
        &body_plan,
        core::slice::from_ref(&planned),
        vec![route.clone()]
    )
    .is_ok());
    assert_eq!(
        AdmittedMaskFormRoutes::new(
            &body_plan,
            core::slice::from_ref(&planned),
            vec![SealedMaskFormRoute {
                placement_ids: vec![conduit_core::PlacementId::from("placement/invented")],
                ..route
            }],
        ),
        Err(MaskRouteAdmissionError::MissingPlacement)
    );

    assert_eq!(
        planned.plan.checked_form_id,
        mask.form_identity.checked_form_id
    );
    assert_eq!(
        planned.plan.expanded_form_id,
        mask.form_identity.expanded_form_id
    );
    assert_eq!(planned.show_placement().gear_id, mask.show_output.gear_id);

    let body = conduit_body::Body::born(
        mask.form_identity.source_document_id.clone(),
        mask.form_identity.checked_form_id.clone(),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap();
    let (body, wake) = body.wake(1, SignId::from("sign/body-woke")).unwrap();
    let presentation = Presentation::new(
        1,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(mask.form_identity.source_document_id.clone()),
            checked_form_id: Some(mask.form_identity.checked_form_id.clone()),
            expanded_form_id: Some(mask.form_identity.expanded_form_id.clone()),
            plan_id: Some(PlanId::from("plan/application-face-source")),
            active_play_id: None,
            sign_ids: vec![SignId::from("sign/presentation")],
        },
        vec![PresentationSubject {
            identity: "mask/form".into(),
            role: PresentationRole::Form,
            name: "Browser graphical Mask Form".into(),
        }],
        vec![],
        vec![],
        vec![PresentationText {
            subject: "mask/form".into(),
            text: "One ordinary Form presents this Face".into(),
        }],
    )
    .unwrap();
    let terminal = planned.show_placement();
    let active_play = bind_active_play(&plan.plan_id, &terminal.host_id, &terminal.boot_id, 1);
    let show = MaskShow::prepared(
        &planned,
        &presentation,
        active_play,
        "mask/form".into(),
        "browser/document".into(),
        SignId::from("sign/show-prepared"),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        SignId::from("sign/show-available"),
    )
    .unwrap();
    show.validate(&presentation).unwrap();
    assert_eq!(show.mask_form, mask.form_identity);
    assert_eq!(
        show.presentation_plan_id,
        PlanId::from("plan/application-face-source")
    );
    assert_ne!(show.presentation_plan_id, show.planned_mask.plan.plan_id);
    assert_eq!(show.planned_mask.plan.plan_id, plan.plan_id);
    assert_eq!(
        presentation.basis.plan_id.as_ref().unwrap().as_str(),
        "plan/application-face-source"
    );
    assert_ne!(
        presentation.basis.plan_id.as_ref(),
        Some(&show.planned_mask.plan.plan_id),
        "the application Plan that produced Face truth remains distinct from the Mask Plan that realized its Show"
    );
}
