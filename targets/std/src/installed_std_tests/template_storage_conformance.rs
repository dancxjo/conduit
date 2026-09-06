use super::{host, installed_std, RecordingTimer};
use conduit_core::{
    BaseImplementationId, ConfigurationValue, ResourceClassId, ResourceOffer, ResourcePoolId,
    StructuredInfoValue,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

const SOURCE_KIND: &str = "conduit-test/template-command";
const SINK_KIND: &str = "conduit-test/template-result";

#[test]
fn reusable_named_template_storage_requires_a_slot_and_executes_without_play_allocations() {
    let pattern = conduit_semantic_catalog::normalized_value(&[250_000, 1_000_000]).unwrap();
    let command =
        conduit_semantic_catalog::put_template_command("protocol-cadence", pattern).unwrap();
    let result = conduit_semantic_catalog::stored_template_result("protocol-cadence").unwrap();
    let source_offer = fixture_offer(&command, conduit_core::PortDirection::Output);
    let sink_offer = fixture_offer(&result, conduit_core::PortDirection::Input);
    let (startup, profile) = catalogs(&command, &result, &source_offer, &sink_offer);
    let source = format!(
        "form named-template-storage (\n    > command: NamedPatternTemplateCommand...|\n    result: NamedPatternTemplateResult...| >\n) {{\n    storage: storage/named-pattern-templates(maximum-commands = 4)\n    command > storage.command\n    storage.result > result\n}}\nform protocol-template-proof {{\n    command: {SOURCE_KIND}(value = \"{}\")\n    storage: named-template-storage\n    result: {SINK_KIND}(value = \"{}\")\n    command.output > storage.command\n    storage.result > result.input\n}}\n",
        hex(&command.canonical_bytes().unwrap()),
        hex(&result.canonical_bytes().unwrap()),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "protocol-template-proof", &profile).unwrap();

    let mut advertisement = host("template-storage-host").advertisement().clone();
    advertisement.capabilities.extend([
        source_offer,
        conduit_std_offers::template_storage_std_offer(),
        sink_offer,
    ]);
    advertisement.resources.push(ResourceOffer {
        content: None,
        pool_id: ResourcePoolId::from("pool/template-storage"),
        class_id: ResourceClassId::from(conduit_std_offers::TEMPLATE_STORAGE_RESOURCE_CLASS),
        capacity_units: 1,
        compute: None,
    });
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();

    let mut unavailable = advertisement.clone();
    unavailable.resources.clear();
    assert!(conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &[unavailable],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        options(&connection_bases, &line_candidates),
    )
    .is_err());

    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        options(&connection_bases, &line_candidates),
    )
    .unwrap();
    let storage = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::TEMPLATE_STORAGE_KIND
        })
        .unwrap();
    assert_eq!(storage.resources.len(), 1);
    assert_eq!(
        storage.resources[0].class_id.as_str(),
        conduit_std_offers::TEMPLATE_STORAGE_RESOURCE_CLASS
    );

    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let mut sign_sequence = 0;
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
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect("template storage executes through the production kernel");
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(kernel.identity.lengths(), (1, 0, 1));
}

fn options<'a>(
    connection_bases: &'a BTreeMap<
        (conduit_core::GearId, conduit_core::GearId),
        BaseImplementationId,
    >,
    line_candidates: &'a BTreeMap<
        (conduit_core::GearId, conduit_core::GearId),
        Vec<conduit_core::LineId>,
    >,
) -> conduit_planner::PlanningOptions<'a> {
    conduit_planner::PlanningOptions {
        connection_bases,
        line_candidates,
        connection_item_capacity: 4,
        connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    }
}

fn catalogs(
    command: &StructuredInfoValue,
    result: &StructuredInfoValue,
    source_offer: &conduit_core::CapabilityOffer,
    sink_offer: &conduit_core::CapabilityOffer,
) -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_sequence_normalization_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_pattern_comparison_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_template_storage_catalogs(&mut startup, &mut profile)
        .unwrap();
    for (kind, value, offer) in [
        (SOURCE_KIND, command, source_offer),
        (SINK_KIND, result, sink_offer),
    ] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![StartupParameterSignature {
                    name: "value".into(),
                    value_type: "Text".into(),
                    default: None,
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
                    default_value: ConfigurationValue::Text(hex(&value.canonical_bytes().unwrap())),
                    validation: ConfigurationRule::TextBytes {
                        maximum: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
                    },
                }],
            })
            .unwrap();
    }
    (startup, profile)
}

fn fixture_offer(
    value: &StructuredInfoValue,
    direction: conduit_core::PortDirection,
) -> conduit_core::CapabilityOffer {
    let mut offer = installed_std::test_structured_selector::offer_named(
        value.value_type(),
        direction,
        SOURCE_KIND,
        SINK_KIND,
    );
    offer.startup_parameters[0].has_default = false;
    offer
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
