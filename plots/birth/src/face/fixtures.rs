//! Canonically checked/planned Mask fixture, not a product effect acknowledgement.
//! Uses the same ordinary Mask construction as presentation interaction conformance.
use alloc::{format, vec, vec::Vec};
use conduit_core::*;
use conduit_plot::{
    KindSignature, ProfileCatalog, StartupCatalog, check_syntax_document,
    expand_canonical_plot_for_authoring, parse_syntax_document,
};
use conduit_presentation::*;
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

/// The explicit Available transition simulates acknowledgement for validation
/// tests only; this fixture proves neither delivery nor physical output.
pub(super) fn prepared_fixture_show(face: &Presentation) -> MaskShow {
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
    install_mask_plot_value_aliases(&mut startup).unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(
            "plot browser (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, "browser", &profiles).unwrap();
    let mask = MaskPlot::admit(&authoring).unwrap();
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
    let empty_bases = alloc::collections::BTreeMap::new();
    let empty_lines = alloc::collections::BTreeMap::new();
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
    let planned = PlannedMaskPlot::admit(&mask, &plan).unwrap();
    let terminal = planned.show_placement();
    let active = bind_active_play(&plan.plan_id, &terminal.host_id, &terminal.boot_id, 1);
    MaskShow::prepared(
        &planned,
        face,
        active,
        face.subjects[0].identity.clone(),
        "fixture/birth-mask-output".into(),
        SignId::from("interaction/show-prepared"),
    )
    .unwrap()
}

/// Fixture acknowledgement only; production must observe its actual effect.
pub(super) fn acknowledged_fixture_show(face: &Presentation) -> MaskShow {
    prepared_fixture_show(face)
        .transition(
            ManifestationLifecycle::Available,
            SignId::from("fixture/show-acknowledged"),
        )
        .unwrap()
}

pub(super) fn producer_plan() -> Plan {
    let kind = mask_kind(
        "test/birth-face",
        vec![],
        vec![mask_port(
            "face",
            PRESENTATION_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Value,
        )],
    );
    let mut catalog = ProfileCatalog::new();
    catalog.insert_kind(kind.clone()).unwrap();
    let plot = conduit_plot::parse(
        "plot birth_producer {\n source: test/birth-face\n}\n",
        &catalog,
    )
    .unwrap();
    let offer = BackOfferBuilder::new(
        kind,
        Back {
            capability_id: "fixture/birth-face".into(),
            execution_profile_id: "fixture/birth-face@1".into(),
            implementation_id: "fixture/birth-face".into(),
            artifact_id: "fixture/birth-face".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/birth".into(),
        boot_id: "boot/birth".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/birth@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let placements =
        conduit_planner::default_placements(&plot, core::slice::from_ref(&host)).unwrap();
    conduit_planner::plan(&plot, &[host], &placements, &[]).unwrap()
}
