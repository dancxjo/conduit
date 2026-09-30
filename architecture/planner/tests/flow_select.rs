use conduit_core::{
    kind_id, port_id, verify_plan, ArtifactId, BaseImplementationId, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, KindIdentity, OfferGeneration, PortDescriptor, PortDirection, PortTemporal,
    BOOL_INFO_ID, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_activations, PlanningOptions,
};
use std::collections::BTreeMap;

const SOURCE: &str = "
form text/is-useful (
 >> value: Text
 accepted: Boolean <= 21B >>
) {
 predicate: test/predicate
 value >> predicate.value
 predicate.accepted >> accepted
}

form flow/select (
 item: type
 predicate: kind (
  >> value: item
  accepted: Boolean <= 21B >>
 )
 >> values: item...|
 selected: item...| >>
) {
 selection: select(maximum-items = 4) predicate()
 values >> selection.value
 selection.selected >> selected
}

form main {
 selection: flow/select(item = Text, predicate = text/is-useful)
}
";

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
        abnormal_kind: None,
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "test/predicate".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = ProfileCatalog::new();
    profile
        .insert_kind(conduit_core::Kind {
            startup_parameters: vec![],
            shorthand: None,
            kind_id: kind_id("test/predicate"),
            kind_contract_revision: KindIdentity::from("test/predicate@1"),
            inputs: vec![port(
                "value",
                "value/text",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            outputs: vec![port(
                "accepted",
                BOOL_INFO_ID,
                PortDirection::Output,
                PortTemporal::Value,
            )],
            configuration: vec![],
            semantic_laws: vec![conduit_core::KindSemanticLaw::ValueContracts(vec![
                conduit_core::FrontValueContract {
                    location: conduit_core::FrontValueLocation::Input(port_id("value")),
                    contract: conduit_core::CheckedValueContract::new(
                        kind_id("value/text"),
                        256,
                        vec![],
                    )
                    .unwrap(),
                },
                conduit_core::FrontValueContract {
                    location: conduit_core::FrontValueLocation::Output(port_id("accepted")),
                    contract: conduit_core::CheckedValueContract::new(
                        kind_id(BOOL_INFO_ID),
                        21,
                        vec![],
                    )
                    .unwrap(),
                },
            ])],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 256,
            },
        })
        .unwrap();
    (startup, profile)
}

fn capability(
    id: &str,
    kind: &str,
    revision: &str,
    input: PortDescriptor,
    output: PortDescriptor,
) -> conduit_core::CapabilityOffer {
    let semantic_contract = if kind == "flow/select" {
        let item =
            conduit_core::CheckedValueContract::new(input.value_kind.clone(), 256, vec![]).unwrap();
        conduit_core::flow_select_activation_contract(
            &item,
            None,
            input.port_id.clone(),
            output.port_id.clone(),
            4,
        )
    } else if kind == "test/predicate" {
        conduit_core::KindSemanticContract {
            configuration: vec![],
            laws: vec![conduit_core::KindSemanticLaw::ValueContracts(vec![
                conduit_core::FrontValueContract {
                    location: conduit_core::FrontValueLocation::Input(input.port_id.clone()),
                    contract: conduit_core::CheckedValueContract::new(
                        input.value_kind.clone(),
                        256,
                        vec![],
                    )
                    .unwrap(),
                },
                conduit_core::FrontValueContract {
                    location: conduit_core::FrontValueLocation::Output(output.port_id.clone()),
                    contract: conduit_core::CheckedValueContract::new(
                        output.value_kind.clone(),
                        21,
                        vec![],
                    )
                    .unwrap(),
                },
            ])],
        }
    } else {
        Default::default()
    };
    conduit_core::capability_offer_from_parts! {
        semantic_contract: semantic_contract,
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
        limits: CapabilityLimits { max_active_instances: 2, max_queue_items: 4, max_queue_bytes: 512 },
    }
}

fn host() -> HostAdvertisement {
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
                "predicate",
                "test/predicate",
                "test/predicate@1",
                port(
                    "value",
                    "value/text",
                    PortDirection::Input,
                    PortTemporal::Value,
                ),
                port(
                    "accepted",
                    BOOL_INFO_ID,
                    PortDirection::Output,
                    PortTemporal::Value,
                ),
            ),
            capability(
                "flow-select",
                "flow/select",
                "conduit.flow/select@1",
                port(
                    "value",
                    "value/text",
                    PortDirection::Input,
                    PortTemporal::Flow { closes: true },
                ),
                port(
                    "selected",
                    "value/text",
                    PortDirection::Output,
                    PortTemporal::Flow { closes: true },
                ),
            ),
        ],
    }
}

#[test]
fn authored_select_seals_the_exact_value_to_boolean_predicate_plan() {
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
    let [activation] = plan.activations.as_slice() else {
        panic!("select must seal exactly one predicate activation")
    };
    let conduit_core::PlannedActivationEntry::Unary(activation) = activation else {
        panic!("flow/select must retain a unary activation")
    };
    assert_eq!(activation.input.value_kind.as_str(), "value/text");
    assert_eq!(activation.output.value_kind.as_str(), BOOL_INFO_ID);
    assert_eq!(activation.limits.maximum_active, 1);
    assert_eq!(activation.limits.maximum_queue_items, 1);
    assert_eq!(activation.limits.maximum_items, 4);
    assert!(activation
        .selected_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .any(|placement| placement.kind_id == kind_id("test/predicate")));
}
