use conduit_core::{
    kind_id, port_id, verify_plan, ArtifactId, BaseImplementationId, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, KindIdentity, OfferGeneration, PlannedActivationEffectMultiplicity,
    PortDescriptor, PortDirection, PortTemporal, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, KindProjection, KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_activations, PlanningOptions,
};
use std::collections::BTreeMap;

const SOURCE: &str = "
form text/normalize (
 >> value: Text
 mapped: Text >>
) {
 normalize: test/normalize
 value >> normalize.value
 normalize.mapped >> mapped
}

form flow/each (
 item: type
 result: type
 transform: kind (
  >> value: item
  mapped: result >>
 )
 >> values: item...|
 mapped: result...| >>
) {
 each: activate transform()
 values >> each.value
 each.mapped >> mapped
}

form main {
 each: flow/each(item = Text, result = Text, transform = text/normalize)
}
";

fn value_port(name: &str, direction: PortDirection, temporal: PortTemporal) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("value/text"),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "test/normalize".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/normalize"),
            kind_contract_revision: KindIdentity::from("test/normalize@1"),
            inputs: vec![value_port(
                "value",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            outputs: vec![value_port(
                "mapped",
                PortDirection::Output,
                PortTemporal::Value,
            )],
            configuration: vec![],
        })
        .unwrap();
    (startup, profile)
}

fn host() -> HostAdvertisement {
    let capability =
        |id: &str, kind: &str, revision: &str, input: PortDescriptor, output: PortDescriptor| {
            conduit_core::capability_offer_from_parts! {
                semantic_contract: Default::default(),
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from(id),
                kind_id: kind_id(kind),
                kind_contract_revision: KindIdentity::from(revision),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from(format!("test/{id}@1")),
                    implementation_id: ImplementationId::from(format!("test/{id}")),
                    artifact_id: ArtifactId::from(format!("test/{id}-artifact")),
                },
                inputs: vec![input],
                outputs: vec![output],
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 2,
                    max_queue_items: 3,
                    max_queue_bytes: 512,
                },
            }
        };
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host"),
        boot_id: BootId::from("boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![
            capability(
                "normalize",
                "test/normalize",
                "test/normalize@1",
                value_port("value", PortDirection::Input, PortTemporal::Value),
                value_port("mapped", PortDirection::Output, PortTemporal::Value),
            ),
            capability(
                "flow-each",
                "flow/each",
                "conduit.flow/each@1",
                value_port(
                    "value",
                    PortDirection::Input,
                    PortTemporal::Flow { closes: true },
                ),
                value_port(
                    "mapped",
                    PortDirection::Output,
                    PortTemporal::Flow { closes: true },
                ),
            ),
        ],
    }
}

#[test]
fn authored_each_plans_one_exact_ordinary_child_plan() {
    let (startup, profile) = catalogs();
    let document = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let authoring = expand_canonical_form_for_authoring(&document, "main", &profile).unwrap();
    let hosts = [host()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty_bases,
        line_candidates: &empty_lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 512,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let plan = plan_expanded_canonical_with_activations(
        &document,
        &authoring.expanded,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        options,
    )
    .unwrap();

    assert!(verify_plan(&plan));
    assert_eq!(plan.activations.len(), 1);
    let activation = &plan.activations[0];
    assert_eq!(
        activation.effect_multiplicity,
        PlannedActivationEffectMultiplicity::OncePerAcceptedInput
    );
    assert_eq!(activation.limits.maximum_active, 1);
    assert_eq!(activation.limits.maximum_queue_items, 1);
    assert!(activation
        .selected_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .any(|placement| placement.kind_id == kind_id("test/normalize")));
    assert_eq!(
        activation
            .selected_plan
            .fragments
            .iter()
            .map(|fragment| fragment.fore_ports.len())
            .sum::<usize>(),
        2
    );
}
