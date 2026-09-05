use super::{host, installed_std, RecordingTimer};
use conduit_core::{
    kind_id, BaseImplementationId, BoundedResourceRef, ConfigurationValue, PortDirection,
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
    let expected_value =
        conduit_semantic_catalog::image_text_record_value(&expected, &image_profile).unwrap();

    let image_type = conduit_semantic_catalog::image_observation_reference_type();
    let record_type = conduit_semantic_catalog::image_text_record_type();
    let mut source_offer =
        installed_std::test_structured_selector::offer(&image_type, PortDirection::Output);
    let mut sink_offer =
        installed_std::test_structured_selector::offer(&record_type, PortDirection::Input);
    source_offer.outputs[0].temporal = PortTemporal::Value;
    sink_offer.inputs[0].temporal = PortTemporal::Value;

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_human_media_catalogs(&mut startup, &mut profile).unwrap();
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
        "form talking-polaroid-kernel {{\n image: conduit-test/structured-source(value = \"{image_hex}\")\n caption: conduit-test/image-caption-source(value = \"{}\")\n compose: media/compose-image-text\n sink: conduit-test/structured-sink(value = \"{expected_hex}\")\n image > compose.image\n caption > compose.caption\n compose.record > sink\n}}\n",
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
    assert_eq!(kernel.identity.lengths(), (2, 0, 1));
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert!(timer.waits.is_empty());
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
