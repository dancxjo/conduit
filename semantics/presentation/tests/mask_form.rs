#![cfg(feature = "form-catalog")]

use conduit_core::{
    bind_active_play, kind_id, port_id, ArtifactId, Back, BackOfferBuilder, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, Kind, KindIdentity, OfferGeneration, PortDescriptor, PortDirection,
    PortTemporal, SignId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    ManifestationLifecycle, MaskForm, MaskShow, PlannedMaskForm, Presentation, PresentationBasis,
    PresentationRole, PresentationSubject, PresentationText, MANIFESTATION_VALUE_KIND,
    PRESENTATION_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND,
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
                MANIFESTATION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "display/input",
            vec![],
            vec![port(
                "interaction",
                PRESENTATION_INTERACTION_VALUE_KIND,
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
                MANIFESTATION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "web/input",
            vec![],
            vec![port(
                "interaction",
                PRESENTATION_INTERACTION_VALUE_KIND,
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
                MANIFESTATION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "audio/listen",
            vec![],
            vec![port(
                "interaction",
                PRESENTATION_INTERACTION_VALUE_KIND,
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
    startup
        .insert_value_kind_alias("Presentation", kind_id(PRESENTATION_VALUE_KIND))
        .unwrap();
    startup
        .insert_value_kind_alias(
            "FaceInteraction",
            kind_id(PRESENTATION_INTERACTION_VALUE_KIND),
        )
        .unwrap();
    startup
        .insert_value_kind_alias("Show", kind_id(MANIFESTATION_VALUE_KIND))
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

#[test]
fn graphical_browser_and_spoken_masks_are_ordinary_forms_with_one_role_boundary() {
    let native = admit(
        "form native-graphical (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n layout: presentation/layout\n compose: presentation/composite\n output: display/show\n input: display/input\n presentation >> layout.presentation\n layout.scene >> compose.scene\n compose.frame >> output.frame\n output.show >> show\n input.interaction >> interaction\n}\n",
        "native-graphical",
    );
    let browser = admit(
        "form browser-graphical (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n presentation >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n",
        "browser-graphical",
    );
    let spoken = admit(
        "form spoken (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n language: presentation/aural\n voice: speech/synthesize\n output: audio/play\n input: audio/listen\n presentation >> language.presentation\n language.text >> voice.text\n voice.audio >> output.audio\n output.show >> show\n input.interaction >> interaction\n}\n",
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
        assert_eq!(
            mask.presentation_input.front_port_id.as_str(),
            "presentation"
        );
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
        "form tutorial (\n >> presentation: Presentation\n show: Show >>\n) {\n output: web/dom\n presentation >> output.presentation\n output.show >> show\n}\n",
    );
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form_for_authoring(&checked, "tutorial", &profiles).unwrap();
    assert!(MaskForm::admit(&expanded).is_err());
}

#[test]
fn the_ordinary_planner_seals_the_mask_form_without_a_mask_planner() {
    let source = "form browser-graphical (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n presentation >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n";
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
    let plan =
        conduit_planner::plan_expanded_canonical(&authoring.expanded, &[host], &placements, &[])
            .unwrap();
    let planned = PlannedMaskForm::admit(&mask, &plan).unwrap();

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
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: vec![SignId::from("sign/presentation")],
        },
        vec![PresentationSubject {
            identity: "mask/form".into(),
            role: PresentationRole::Form,
            label: "Browser graphical Mask Form".into(),
            accessibility_name: "Browser graphical Mask Form".into(),
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
    assert_eq!(show.planned_mask.plan.plan_id, plan.plan_id);
}
