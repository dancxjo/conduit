//! Exact model adoption and authored numeric export setup; no execution driver.
use conduit_ai::{integer_categorical_step::PreparedCategoricalStep, *};
use conduit_composite::KernelCompositeDefinition;
use conduit_core::*;
use conduit_language::parser_model::*;
use std::{collections::BTreeMap, sync::Arc};
pub fn profile() -> Arc<PreparedCategoricalStep> {
    let (artifact, signature) = english_parser_scorer_descriptor();
    let reference = &artifact.content;
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "test/round-model".into(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: "test/round-model-read".into(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: Some(1),
        availability: ResourceReferenceAvailability::Available,
    };
    let residence = ResourceContentOffer {
        contract: ResourceContentRequirement {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile.clone(),
            maximum_bytes: ENGLISH_PARSER_MODEL_BYTES as u32,
            maximum_items: 1,
            retention: ResourceRetention::Play,
            sharing: ResourceSharing::ImmutableReadMany,
            access: ResourceAccessMode::ReadPublished,
            generation_slots: 1,
            reader_leases: 16,
            publication_slots: 0,
            sensitive: false,
        },
        owner_host: "parser-model-test".into(),
        owner_boot: "parser-model-test-boot".into(),
        base_id: "parser-model-test-base".into(),
        residence_profile: "test/owned-immutable-memory@1".into(),
    };
    let resource = Arc::new(
        AdmittedModelResource::adopt(
            artifact,
            signature,
            Arc::from(ENGLISH_PARSER_WEIGHTS),
            &binding,
        )
        .unwrap(),
    );
    Arc::new(
        prepare_english_parser_scorer(Some(resource), "test/parser-model-pool".into(), residence)
            .unwrap(),
    )
}
pub fn definition(
    profile: &PreparedCategoricalStep,
) -> (
    conduit_plot::CheckedSyntaxDocument,
    conduit_plot::ProfileCatalog,
    KernelCompositeDefinition,
) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut catalog = conduit_plot::ProfileCatalog::new();
    profile.install(&mut startup, &mut catalog, true).unwrap();
    let input = profile.indices_type().profile().unwrap();
    let output = profile.scores_type().profile().unwrap();
    let input_max =
        conduit_plot::maximum_prepared_canonical_value_bytes(profile.indices_type()).unwrap();
    let output_max =
        conduit_plot::maximum_prepared_canonical_value_bytes(profile.scores_type()).unwrap();
    let source = format!(
        "plot parser-round-numeric (\n >> input: {}...| <= {}B\n output: {}...| <= {}B >>\n) {{\n model: {}\n input >> model.indices\n model.scores >> output\n}}\n",
        input.value_kind().as_str(), input_max, output.value_kind().as_str(), output_max, profile.kind_identity(true));
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "parser-model-test".into(),
        boot_id: "parser-model-test-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "test/host@1".into(),
        bases: vec![],
        resources: vec![profile.resource_offer().clone()],
        planner_capabilities: vec![],
        capabilities: vec![profile.offer(true).unwrap()],
    }];
    let syntax = conduit_plot::parse_syntax_document(&source);
    let checked =
        conduit_plot::check_syntax_document_with_literal_constructors(&syntax, &startup, &catalog)
            .unwrap();
    let authoring = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "parser-round-numeric",
        &catalog,
    )
    .unwrap();
    let placements =
        conduit_planner::default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let options = conduit_planner::PlanningOptions {
        connection_bases: &empty_bases,
        line_candidates: &empty_lines,
        connection_item_capacity: 1,
        connection_byte_capacity: input_max.max(output_max),
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let bounds = BTreeMap::from([
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("input"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: input_max,
            },
        ),
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("output"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: output_max,
            },
        ),
    ]);
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        options,
        &bounds,
    )
    .unwrap();
    let fores = &plan.fragments[0].fore_ports;
    assert_eq!(
        fores.len(),
        2,
        "both actual Fore queues must be Plan sealed"
    );
    assert!(fores.iter().all(|fore| fore.item_capacity == 1));
    assert!(fores
        .iter()
        .any(|fore| fore.direction == PortDirection::Input && fore.byte_capacity == input_max));
    assert!(fores
        .iter()
        .any(|fore| fore.direction == PortDirection::Output && fore.byte_capacity == output_max));
    let definition = KernelCompositeDefinition::from_canonical_export(
        "parser-round-channel".into(),
        "parser-round-channel-boot".into(),
        OfferGeneration(1),
        "parser/channel@1".into(),
        "parser/channel@1".into(),
        "parser/channel-artifact@1".into(),
        &checked,
        "parser-round-numeric",
        &catalog,
        plan,
        FailureReason::CompositeCapabilityFailed,
    )
    .unwrap();
    (checked, catalog, definition)
}
