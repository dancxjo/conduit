use super::{host, installed_std, RecordingTimer};
use conduit_core::{
    kind_id, process_owned_line_offer_with_limits, BaseImplementationId, BoundedResourceRef,
    CapabilityId, ConfigurationValue, GearId, LineScope, LineSecurity, LinkLimits, PortDirection,
    PortTemporal, ResourceClassId, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

#[test]
fn authored_image_text_runs_through_planner_and_production_kernel() {
    let image_profile = kind_id("media/image-rgba8@1");
    let image = conduit_human::ImageObservationReference::new(
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([21; 32]),
            content_profile: image_profile.clone(),
            access_class: ResourceClassId::from("conduit.resource/portable-content@1"),
            extent: ResourceExtent {
                bytes: 4096,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([22; 32]),
                expires_at: None,
            },
        },
        640,
        480,
        &image_profile,
    )
    .unwrap();
    let image_value = conduit_semantic_catalog::image_observation_value(&image).unwrap();
    let expected = conduit_human::compose_image_text(
        &image_profile,
        image,
        "Inspection point A".into(),
        vec![],
    )
    .unwrap();
    let record_value =
        conduit_semantic_catalog::image_text_record_value(&expected, &image_profile).unwrap();
    let typed_value = conduit_net::typed_record_value(&record_value).unwrap();
    let mut frame = [0; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES];
    let written = conduit_net::frame_typed_record_value_into(&typed_value, &mut frame).unwrap();
    let expected_value = conduit_net::framed_typed_record_value(&frame[..written]).unwrap();

    let image_type = conduit_semantic_catalog::image_observation_reference_type();
    let record_type = conduit_net::framed_typed_record_type();
    let mut source_offer =
        installed_std::test_structured_selector::offer(&image_type, PortDirection::Output);
    let mut sink_offer =
        installed_std::test_structured_selector::offer(&record_type, PortDirection::Input);
    source_offer.outputs[0].temporal = PortTemporal::Value;
    sink_offer.inputs[0].temporal = PortTemporal::Value;

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_human_media_catalogs(&mut startup, &mut profile).unwrap();
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile).unwrap();
    install_fixture(&mut startup, &mut profile, &source_offer);
    let mut caption_offer = installed_std::test_structured_selector::raw_source_offer(
        "conduit-test/image-caption-source",
        "value/text@1",
    );
    caption_offer.outputs[0].temporal = PortTemporal::Value;
    install_fixture(&mut startup, &mut profile, &caption_offer);
    install_fixture(&mut startup, &mut profile, &sink_offer);

    let image_hex = hex(&image_value.canonical_bytes().unwrap());
    let expected_hex = hex(&expected_value.canonical_bytes().unwrap());
    let source = format!(
        "form talking-polaroid-kernel {{\n image: conduit-test/structured-source(value = \"{image_hex}\")\n caption: conduit-test/image-caption-source(value = \"{}\")\n compose: media/compose-image-text\n adapt: media/image-text-to-typed-record\n frame: record/frame-typed\n sink: conduit-test/structured-sink(value = \"{expected_hex}\")\n image > compose.image\n caption > compose.caption\n compose.record > adapt.record\n adapt.typed > frame.record\n frame.frame > sink\n}}\n",
        hex(b"Inspection point A")
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "talking-polaroid-kernel", &profile).unwrap();

    let mut advertisement = host("image-text-host").advertisement().clone();
    advertisement
        .capabilities
        .extend([source_offer, caption_offer, sink_offer]);
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4_096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let mut output = Vec::with_capacity(1024);
    let mut timer = RecordingTimer { waits: vec![] };
    let report = installed_std::run_fragment(
        installed_std::InstalledRunHost {
            advertisement: &advertisement,
            playback: None,
            midi_input: None,
            midi_output: None,
            keyboard: None,
            local_model: None,
            vector_search: None,
            calendar: None,
        },
        &plan.fragments[0],
        0,
        &mut 0,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .unwrap();
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(kernel.identity.lengths(), (4, 0, 1));
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert!(timer.waits.is_empty());
}

#[test]
fn authored_image_text_plans_an_exact_remote_framed_record_session() {
    let image_profile = kind_id("media/image-rgba8@1");
    let image = conduit_human::ImageObservationReference::new(
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([31; 32]),
            content_profile: image_profile.clone(),
            access_class: ResourceClassId::from("conduit.resource/portable-content@1"),
            extent: ResourceExtent {
                bytes: 4096,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([32; 32]),
                expires_at: None,
            },
        },
        640,
        480,
        &image_profile,
    )
    .unwrap();
    let image_value = conduit_semantic_catalog::image_observation_value(&image).unwrap();
    let record = conduit_human::compose_image_text(
        &image_profile,
        image,
        "Inspection point B".into(),
        vec![],
    )
    .unwrap();
    let record_value =
        conduit_semantic_catalog::image_text_record_value(&record, &image_profile).unwrap();
    let typed_value = conduit_net::typed_record_value(&record_value).unwrap();
    let mut framed = [0; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES];
    let framed_len = conduit_net::frame_typed_record_value_into(&typed_value, &mut framed).unwrap();
    let expected = conduit_net::framed_typed_record_value(&framed[..framed_len])
        .unwrap()
        .canonical_bytes()
        .unwrap();

    let image_type = conduit_semantic_catalog::image_observation_reference_type();
    let framed_type = conduit_net::framed_typed_record_type();
    let mut image_source =
        installed_std::test_structured_selector::offer(&image_type, PortDirection::Output);
    let mut record_sink =
        installed_std::test_structured_selector::offer(&framed_type, PortDirection::Input);
    image_source.outputs[0].temporal = PortTemporal::Value;
    record_sink.inputs[0].temporal = PortTemporal::Value;
    let mut caption_source = installed_std::test_structured_selector::raw_source_offer(
        "conduit-test/image-caption-source",
        "value/text@1",
    );
    caption_source.outputs[0].temporal = PortTemporal::Value;

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_human_media_catalogs(&mut startup, &mut profile).unwrap();
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile).unwrap();
    install_fixture(&mut startup, &mut profile, &image_source);
    install_fixture(&mut startup, &mut profile, &caption_source);
    install_fixture(&mut startup, &mut profile, &record_sink);

    let source = format!(
        "form talking-polaroid-remote {{\n image: conduit-test/structured-source(value = \"{}\")\n caption: conduit-test/image-caption-source(value = \"{}\")\n compose: media/compose-image-text\n adapt: media/image-text-to-typed-record\n frame: record/frame-typed\n sink: conduit-test/structured-sink(value = \"{}\")\n image > compose.image\n caption > compose.caption\n compose.record > adapt.record\n adapt.typed > frame.record\n frame.frame > sink\n}}\n",
        hex(&image_value.canonical_bytes().unwrap()),
        hex(b"Inspection point B"),
        hex(&expected),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "talking-polaroid-remote", &profile).unwrap();

    let mut source_host = host("image-text-source").advertisement().clone();
    source_host
        .capabilities
        .extend([image_source.clone(), caption_source.clone()]);
    source_host
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let mut sink_host = host("image-text-sink").advertisement().clone();
    sink_host.capabilities.push(record_sink.clone());
    sink_host
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let hosts = [source_host.clone(), sink_host.clone()];
    let placements = conduit_planner::PlacementChoices {
        by_gear: BTreeMap::from([
            placement("image", &source_host, &image_source.capability_id),
            placement("caption", &source_host, &caption_source.capability_id),
            placement_kind(
                "compose",
                &source_host,
                conduit_semantic_catalog::IMAGE_TEXT_COMPOSE_KIND,
            ),
            placement_kind(
                "adapt",
                &source_host,
                conduit_semantic_catalog::IMAGE_TEXT_TYPED_RECORD_KIND,
            ),
            placement_kind("frame", &source_host, conduit_net::TYPED_RECORD_FRAME_KIND),
            placement("sink", &sink_host, &record_sink.capability_id),
        ]),
    };
    let mut line = process_owned_line_offer_with_limits(
        "talking-polaroid/framed-record-line",
        "talking-polaroid/framed-record-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "talking-polaroid/websocket-instance",
        &source_host,
        &sink_host,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 4_096,
            maximum_buffered_bytes: 4_096,
            maximum_frame_bytes: 8_192,
        },
    );
    line.contract.scope = LineScope::LocalNetwork;
    line.contract.security = LineSecurity::PlaintextNetwork;
    let line_candidates = BTreeMap::from([(
        (
            GearId::from("talking-polaroid-remote/frame"),
            GearId::from("talking-polaroid-remote/sink"),
        ),
        vec![line.line_id.clone()],
    )]);
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 4_096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: std::slice::from_ref(&line),
        },
    )
    .unwrap();

    assert_eq!(plan.fragments.len(), 2);
    let source_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == source_host.host_id)
        .unwrap();
    let sink_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == sink_host.host_id)
        .unwrap();
    let connection = source_fragment
        .connections
        .iter()
        .find(|connection| connection.source_port_id.as_str() == "frame")
        .unwrap();
    assert_eq!(
        connection.value_kind,
        framed_type.profile().unwrap().value_kind().clone()
    );
    assert_eq!(
        connection.selected_line.as_ref().unwrap().line_id,
        line.line_id
    );
    assert_eq!(connection.item_capacity, 1);
    assert_eq!(connection.byte_capacity, 4_096);
    assert!(sink_fragment
        .connections
        .iter()
        .any(|candidate| candidate.connection_id == connection.connection_id));

    let binding = conduit_wire::SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source_fragment.fragment_id.clone(),
        sink_fragment.fragment_id.clone(),
        connection,
    )
    .unwrap();
    let mut source_session =
        conduit_wire::SessionMachine::new(binding.clone(), conduit_wire::SessionRole::Source)
            .unwrap();
    let mut sink_session =
        conduit_wire::SessionMachine::new(binding.clone(), conduit_wire::SessionRole::Sink)
            .unwrap();
    admit_session_pair(&binding, &mut source_session, &mut sink_session);

    let offered = binding.frame(conduit_wire::SessionMessage::Offered {
        sequence: 0,
        payload: &expected,
    });
    source_session.admit_outbound(offered).unwrap();
    sink_session.admit_inbound(offered).unwrap();
    let accepted = binding.frame(conduit_wire::SessionMessage::Accepted { sequence: 0 });
    sink_session.admit_outbound(accepted).unwrap();
    source_session.admit_inbound(accepted).unwrap();
    let delivered = binding.frame(conduit_wire::SessionMessage::Delivered { sequence: 0 });
    sink_session.admit_outbound(delivered).unwrap();
    source_session.admit_inbound(delivered).unwrap();
    assert_eq!(source_session.next_sequence(), 1);
    assert_eq!(sink_session.next_sequence(), 1);
}

fn placement(
    gear: &str,
    host: &conduit_core::HostAdvertisement,
    capability_id: &CapabilityId,
) -> (GearId, conduit_planner::PlacementChoice) {
    (
        GearId::from(format!("talking-polaroid-remote/{gear}")),
        conduit_planner::PlacementChoice {
            host_id: host.host_id.clone(),
            capability_id: capability_id.clone(),
        },
    )
}

fn placement_kind(
    gear: &str,
    host: &conduit_core::HostAdvertisement,
    kind: &str,
) -> (GearId, conduit_planner::PlacementChoice) {
    let capability = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id == kind_id(kind))
        .unwrap();
    placement(gear, host, &capability.capability_id)
}

fn admit_session_pair(
    binding: &conduit_wire::SessionBinding,
    source: &mut conduit_wire::SessionMachine,
    sink: &mut conduit_wire::SessionMachine,
) {
    let hello = binding.hello_frame();
    source.admit_outbound(hello).unwrap();
    sink.admit_inbound(hello).unwrap();
    sink.admit_outbound(hello).unwrap();
    source.admit_inbound(hello).unwrap();
    let ready = binding.frame(conduit_wire::SessionMessage::Ready);
    source.admit_outbound(ready).unwrap();
    sink.admit_inbound(ready).unwrap();
    sink.admit_outbound(ready).unwrap();
    source.admit_inbound(ready).unwrap();
    assert!(source.is_active());
    assert!(sink.is_active());
}

fn install_fixture(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    offer: &conduit_core::CapabilityOffer,
) {
    startup
        .insert(KindSignature {
            kind: offer.kind_id.as_str().into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "Text".into(),
                default: Some("".into()),
            }],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: offer.kind_id.clone(),
            kind_contract_revision: offer.kind_contract_revision.clone(),
            inputs: offer.inputs.clone(),
            outputs: offer.outputs.clone(),
            configuration: vec![ConfigurationField {
                key: "value".into(),
                default_value: ConfigurationValue::Text(String::new()),
                validation: ConfigurationRule::TextBytes {
                    maximum: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 2,
                },
            }],
        })
        .unwrap();
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
