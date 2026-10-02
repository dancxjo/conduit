use super::{host, installed_std};
use crate::body_execution::BodyRunRequest;
use conduit_body::{Body, BodyPlan, BodyPlotPlan, BodyWorkset, ResidentPlot};
use conduit_core::{
    kind_id, BaseImplementationId, BoundedResourceRef, ConfigurationValue, PortDirection,
    PortTemporal, ResourceClassId, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity, SignId, StructuredInfoValue, TerminalDisposition,
};
use conduit_kernel::causal_evidence::{
    EvidenceMetadataFact, EvidenceMetadataLookup, EvidenceMetadataVisit,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, KindConfigurationField,
    KindConfigurationRule, KindProjection, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

struct Clock;

impl crate::TimerAdapter for Clock {
    fn wait(&mut self, _: std::time::Duration) {}
}

#[test]
fn canonical_image_text_composition_coexists_in_one_body_play() {
    let image_profile = kind_id("media/image-rgba8@1");
    let image = conduit_human::ImageObservationReference::new(
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([41; 32]),
            content_profile: image_profile.clone(),
            access_class: ResourceClassId::from("conduit.resource/portable-content@1"),
            extent: ResourceExtent {
                bytes: 4096,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([42; 32]),
                expires_at: None,
            },
        },
        640,
        480,
        &image_profile,
    )
    .unwrap();
    let image_value = conduit_semantic_catalog::image_observation_value(&image).unwrap();
    let expected =
        conduit_human::compose_image_text(&image_profile, image, "Body inspection".into(), vec![])
            .unwrap();
    let record =
        conduit_semantic_catalog::image_text_record_value(&expected, &image_profile).unwrap();
    let typed = conduit_net::typed_record_value(&record).unwrap();
    let mut framed = [0; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES];
    let length = conduit_net::frame_typed_record_value_into(&typed, &mut framed).unwrap();
    let expected_frame = conduit_net::framed_typed_record_value(&framed[..length]).unwrap();

    let image_type = conduit_semantic_catalog::image_observation_reference_type();
    let frame_type = conduit_net::framed_typed_record_type();
    let text_type = conduit_net::text_type();
    let mut image_source = installed_std::test_structured_selector::offer_named(
        &image_type,
        PortDirection::Output,
        "conduit-test/body-image-source",
        "conduit-test/unused-image-sink",
    );
    image_source.outputs[0].temporal = PortTemporal::Value;
    let mut caption_source = installed_std::test_structured_selector::raw_source_offer(
        "conduit-test/body-caption-source",
        conduit_net::TEXT_INFO_ID,
    );
    caption_source.outputs[0].temporal = PortTemporal::Value;
    let mut frame_sink = installed_std::test_structured_selector::offer_named(
        &frame_type,
        PortDirection::Input,
        "conduit-test/unused-frame-source",
        "conduit-test/body-frame-sink",
    );
    frame_sink.inputs[0].temporal = PortTemporal::Value;
    let mut status_source = installed_std::test_structured_selector::offer_named(
        &text_type,
        PortDirection::Output,
        "conduit-test/body-status-source",
        "conduit-test/unused-status-sink",
    );
    let mut status_sink = installed_std::test_structured_selector::offer_named(
        &text_type,
        PortDirection::Input,
        "conduit-test/unused-status-source",
        "conduit-test/body-status-sink",
    );

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_human_media_catalogs(&mut startup, &mut profile).unwrap();
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile).unwrap();
    for offer in [
        &mut image_source,
        &mut caption_source,
        &mut frame_sink,
        &mut status_source,
        &mut status_sink,
    ] {
        install_fixture(&mut startup, &mut profile, offer);
    }

    let polaroid_source = format!(
        "{}\nform talking-polaroid-body {{\n image: conduit-test/body-image-source(value = \"{}\")\n caption: conduit-test/body-caption-source(value = \"{}\")\n compose: image-text-compose\n adapt: media/image-text-to-typed-record\n frame: record/frame-typed\n sink: conduit-test/body-frame-sink(value = \"{}\")\n image >> compose.image\n caption >> compose.caption\n compose.record >> adapt.record\n adapt.typed >> frame.record\n frame.frame >> sink\n}}\n",
        include_str!("../../../../plots/image-text-compose/main.conduit"),
        hex(&image_value.canonical_bytes().unwrap()),
        hex(b"Body inspection"),
        hex(&expected_frame.canonical_bytes().unwrap()),
    );
    let status = StructuredInfoValue::leaf(text_type, b"unrelated status".to_vec()).unwrap();
    let unrelated_source = format!(
        "plot unrelated-status {{\n source: conduit-test/body-status-source(value = \"{}\")\n sink: conduit-test/body-status-sink(value = \"{}\")\n source >> sink\n}}\n",
        hex(&status.canonical_bytes().unwrap()),
        hex(&status.canonical_bytes().unwrap()),
    );
    let mut advertisement = host("talking-polaroid-body-host").advertisement().clone();
    advertisement.capabilities.extend([
        image_source,
        caption_source,
        frame_sink,
        status_source,
        status_sink,
    ]);
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let plans = [
        plan(
            &polaroid_source,
            "talking-polaroid-body",
            &startup,
            &profile,
            &advertisement,
        ),
        plan(
            &unrelated_source,
            "unrelated-status",
            &startup,
            &profile,
            &advertisement,
        ),
    ];
    let residents: Vec<_> = plans
        .iter()
        .map(|plan| {
            ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            )
        })
        .collect();
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(residents.clone()).unwrap(),
        1,
        SignId::from("sign/talking-polaroid-body-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/talking-polaroid-body-woke"))
        .unwrap()
        .1;
    let body_plan = BodyPlan::seal(
        &wake,
        residents
            .into_iter()
            .zip(plans)
            .map(|(plot, plan)| BodyPlotPlan { plot, plan })
            .collect(),
    )
    .unwrap();
    let mut std_host = crate::StdHost::from_advertisement(advertisement).unwrap();
    let mut output = Vec::with_capacity(1024);
    let report = std_host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &crate::RunControl::default(),
                keyboard: None,
            },
            &mut output,
            &mut Clock,
        )
        .unwrap();

    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert!(report.failure.is_none());
    assert!(report.cleanup_failure.is_none());
    assert_eq!(report.partitions.len(), 2);
    assert_eq!(report.requests.len(), 4);
    assert!(report.play.validate_for(&body_plan));
    assert!(output.is_empty());

    let evidence =
        crate::body_causal_evidence::BodyRunCausalRecord::from_run(&body_plan, &report).unwrap();
    let retained = evidence.retained_evidence().collect::<Vec<_>>();
    assert!(!retained.is_empty());
    for identity in retained {
        let mut source = None;
        assert_eq!(
            evidence.visit(identity, &mut |fact| {
                if let EvidenceMetadataFact::Source {
                    document,
                    start,
                    end,
                    line,
                    column,
                    end_line,
                    end_column,
                } = fact
                {
                    source = Some((document, start, end, line, column, end_line, end_column));
                }
                true
            }),
            EvidenceMetadataVisit::Visited
        );
        let (document, start, end, line, column, end_line, end_column) = source.unwrap();
        assert!(body_plan
            .plots
            .iter()
            .any(|plot| plot.plan.source_document_id.as_str() == document));
        assert!(start.is_some_and(|value| value < end.unwrap()));
        assert!(line.is_some_and(|value| value <= end_line.unwrap()));
        assert!(column.is_some());
        assert!(end_column.is_some());
    }
}

fn plan(
    source: &str,
    entry: &str,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
    host: &conduit_core::HostAdvertisement,
) -> conduit_core::Plan {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, startup).unwrap();
    let expanded = expand_canonical_plot(&checked, entry, profile).unwrap();
    let hosts = [host.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let limits = expanded
        .connections
        .iter()
        .map(|connection| {
            (
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                },
            )
        })
        .collect();
    conduit_planner::plan_expanded_canonical_with_connection_limits(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
    .unwrap()
}

fn install_fixture(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    offer: &mut conduit_core::CapabilityOffer,
) {
    installed_std::test_structured_selector::bind_text_configuration(
        offer,
        "value",
        String::new(),
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 2,
    );
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
        .insert(KindProjection {
            kind_id: offer.kind_id.clone(),
            kind_contract_revision: offer.kind_contract_revision.clone(),
            inputs: offer.inputs.clone(),
            outputs: offer.outputs.clone(),
            configuration: vec![KindConfigurationField {
                key: "value".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::TextBytes {
                    maximum: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 2,
                },
            }],
        })
        .unwrap();
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
