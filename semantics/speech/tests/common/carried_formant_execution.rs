//! Real checked Source, selected Plan, installed factory, and kernel Play.
use conduit_composite::*;
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::carried_formant::*;
use std::collections::BTreeMap;

pub fn prepare(factory: CarriedFormantFactory) -> (KernelCompositeHost, Plan) {
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("CarriedSpeechSource", kind_id(SOURCE_VALUE_KIND))
        .unwrap();
    startup
        .insert_value_kind_alias("PcmFrames", kind_id(conduit_audio::AUDIO_PCM_INFO_ID))
        .unwrap();
    startup
        .insert(KindSignature {
            kind: KIND.into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = ProfileCatalog::new();
    profile.insert_kind(factory.contract()).unwrap();
    let source = r#"plot proof/carried-formant (
 >> source: CarriedSpeechSource <= 32B
 audio: PcmFrames...| <= 285B >>
) {
 voice: speech/carried-formant()
 source >> voice.source
 voice.audio >> audio
}."#;
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "proof/carried-formant", &profile).unwrap();
    let offer = factory.offer();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "proof/carried-formant".into(),
        boot_id: "proof/carried-formant-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: PROFILE.into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![offer.clone()],
        planner_capabilities: vec![],
    };
    let hosts = [host];
    let choices = conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "source", 32),
        (PortDirection::Output, "audio", 285),
    ]
    .into_iter()
    .map(|(direction, name, byte_capacity)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &choices,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 285,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    let bind = |bindings: &[AuthoringFrontBinding], ports: &[PortDescriptor]| {
        bindings
            .iter()
            .map(|binding| {
                let placement = plan
                    .fragments
                    .iter()
                    .flat_map(|fragment| &fragment.placements)
                    .find(|p| p.gear_id == binding.gear_id)
                    .unwrap();
                KernelCompositeFrontBinding {
                    external_port: ports
                        .iter()
                        .find(|p| p.port_id == binding.front_port_id)
                        .unwrap()
                        .clone(),
                    internal_child: placement.host_id.clone(),
                    internal_placement_id: placement.placement_id.clone(),
                    internal_port_id: binding.gear_port_id.clone(),
                    terminal: CompositeFrontTerminal::Independent,
                }
            })
            .collect()
    };
    // The boundary adapter binds the exact checked authored fronts to selected
    // Plan endpoints; it supplies no speech behavior or replacement source.
    let definition = KernelCompositeDefinition {
        host_id: "proof/carried-formant-wrapper".into(),
        boot_id: "proof/carried-formant-wrapper-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: PROFILE.into(),
        external_capability: offer,
        internal_plan: plan.clone(),
        failure_translation: FailureReason::CompositeCapabilityFailed,
        boundary: KernelCompositeBoundary {
            input_fronts: bind(&authored.input_bindings, authored.front.inputs()),
            output_fronts: bind(&authored.output_bindings, authored.front.outputs()),
        },
    };
    let mut registry = KernelOperationRegistry::new();
    registry.install(factory).unwrap();
    let host = KernelCompositeHost::prepare(definition, &registry).unwrap();
    (host, plan)
}
