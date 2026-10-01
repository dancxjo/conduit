#![allow(dead_code)]

use conduit_body::Body;
use conduit_core::{
    bind_active_play, kind_id, port_id, resource_offer, resource_requirement, ArtifactId, Back,
    BackOfferBuilder, BootId, CapabilityId, CapabilityLimits, ExecutionProfileId,
    HostAdvertisement, HostCallContractId, HostCallRequirement, HostId, HostProfileId,
    ImplementationId, Kind, KindIdentity, OfferGeneration, PortDescriptor, PortDirection,
    PortTemporal, SignId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    install_mask_form_value_aliases, renderer_kind_projection, renderer_offer,
    ManifestationLifecycle, MaskForm, MaskShow, PlannedMaskForm, Presentation, PresentationBasis,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText, RendererRealizationOffer, FACE_INTERACTION_VALUE_KIND,
    MAX_RENDERER_VALUE_BYTES, PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND,
};

pub const WAYLAND_RESOURCE: &str = "conduit.resource/wayland-surface@1";
pub const DOM_RESOURCE: &str = "conduit.resource/browser-document@1";

pub fn checked_renderer_form() -> conduit_form::CheckedForm {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(renderer_kind_projection()).unwrap();
    parse(
        "form patchbay-show {\n    renderer: presentation/renderer\n}\n",
        &catalog,
    )
    .expect("one ordinary portable renderer Front checks")
}

pub fn host(
    host: &str,
    boot: &str,
    capability: &str,
    implementation: &str,
    artifact: &str,
    target: &str,
    resource_class: &str,
) -> HostAdvertisement {
    let limits = CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
    };
    let pool_id = format!("{host}/surface");
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("presentation/host@1"),
        bases: vec![],
        resources: vec![resource_offer(&pool_id, resource_class, 1)],
        capabilities: vec![renderer_offer(RendererRealizationOffer {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from("presentation/renderer-hosted@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_call: HostCallRequirement {
                contract_id: HostCallContractId::from("conduit.host/present@1"),
                target_kind: Some(kind_id(target)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
            },
            resource_requirement: resource_requirement(resource_class, 1),
            limits,
        })],
        planner_capabilities: vec![],
    }
}

pub fn plan_for(form: &conduit_form::CheckedForm, host: HostAdvertisement) -> conduit_core::Plan {
    let placements = default_placements(form, std::slice::from_ref(&host)).unwrap();
    plan(form, &[host], &placements, &[]).unwrap()
}

pub fn presentation(form: &conduit_form::CheckedForm, plan: &conduit_core::Plan) -> Presentation {
    let body = Body::born(
        form.source_document_id.clone(),
        form.checked_form_id.clone(),
        1,
        SignId::from("patchbay/sign/bornd"),
    )
    .unwrap();
    let (body, wake) = body.wake(1, SignId::from("patchbay/sign/woke")).unwrap();
    Presentation::new(
        7,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(form.source_document_id.clone()),
            checked_form_id: Some(form.checked_form_id.clone()),
            expanded_form_id: Some(form.expanded_form_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: vec![SignId::from("patchbay/sign/source")],
        },
        vec![
            PresentationSubject {
                identity: "patchbay/form".into(),
                role: PresentationRole::Form,
                name: "Patchbay Form".into(),
            },
            PresentationSubject {
                identity: "patchbay/renderer".into(),
                role: PresentationRole::Gear,
                name: "Portable presentation renderer".into(),
            },
        ],
        vec![PresentationRelationship {
            source: "patchbay/form".into(),
            target: "patchbay/renderer".into(),
            kind: PresentationRelationshipKind::Contains,
        }],
        vec![],
        vec![PresentationText {
            subject: "patchbay/renderer".into(),
            text: "Presentation to Manifestation".into(),
        }],
    )
    .unwrap()
}

fn mask_port(
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

fn mask_kind(name: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> Kind {
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
            max_active_instances: 2,
            max_queue_items: 2,
            max_queue_bytes: 64 * 1024,
        },
    }
}

/// Plan one ordinary graphical Mask Form and realize an exact available Show.
/// Interaction tests use this instead of constructing the retired generic
/// Manifestation boundary directly.
pub fn available_mask_show(face: &Presentation) -> MaskShow {
    let definitions = vec![
        mask_kind(
            "web/dom",
            vec![mask_port(
                "presentation",
                PRESENTATION_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![mask_port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        mask_kind(
            "web/input",
            vec![],
            vec![mask_port(
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
    let checked = check_syntax_document(
        &parse_syntax_document(
            "form browser (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let authoring = expand_canonical_form_for_authoring(&checked, "browser", &profiles).unwrap();
    let mask = MaskForm::admit(&authoring).unwrap();
    let capabilities = authoring
        .expanded
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
    let mask_host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/interaction-mask"),
        boot_id: BootId::from("boot/interaction-mask"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("mask/test@1"),
        bases: vec![],
        resources: vec![],
        capabilities,
        planner_capabilities: vec![],
    };
    let placements = conduit_planner::default_expanded_placements(
        &authoring.expanded,
        core::slice::from_ref(&mask_host),
    )
    .unwrap();
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
    let empty_bases = std::collections::BTreeMap::new();
    let empty_lines = std::collections::BTreeMap::new();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &[mask_host],
        &placements,
        &[],
        conduit_planner::PlanningOptions {
            connection_bases: &empty_bases,
            line_candidates: &empty_lines,
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
    let terminal = planned.show_placement();
    let active = bind_active_play(&plan.plan_id, &terminal.host_id, &terminal.boot_id, 1);
    MaskShow::prepared(
        &planned,
        face,
        active,
        "patchbay/form".into(),
        "display/interaction-test".into(),
        SignId::from("interaction/show-prepared"),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        SignId::from("interaction/show-available"),
    )
    .unwrap()
}
