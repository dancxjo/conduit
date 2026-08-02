use conduit_compile::{
    BudgetDocument, InstalledHostObservationInput, InstalledProfile, PinDocument,
    ReportCapabilityDocument, compile_source,
};
use conduit_core::SemanticHash;
use conduit_net::{
    NATIVE_USERSPACE_ROUTE_IMPLEMENTATION_ID, install_native_userspace_route_implementation,
    native_userspace_route_capability_requirement, register_deterministic_network_providers,
};
use conduit_runtime::{InstalledCapabilityRequirement, Registry};

const SOURCE: &str = include_str!("../../../examples/standing-network-packet-path.panel");
const REFERENCE_ROUTE_IMPLEMENTATION_ID: &str = "conduit.net/packet-router-reference";

fn host(name: &str) -> InstalledHostObservationInput {
    let mut observation = InstalledHostObservationInput::conduct_host();
    observation.id = format!("conduit/observation/{name}");
    observation.host = format!("conduit/host/{name}");
    observation.boot_id = format!("conduit/boot/{name}");
    observation.time_basis = format!("clock/{name}");
    observation
}

fn observed_capability(requirement: InstalledCapabilityRequirement) -> ReportCapabilityDocument {
    ReportCapabilityDocument {
        interface: PinDocument {
            id: requirement.interface.id.to_string(),
            schema_version: requirement.interface.schema_version,
            semantic_hash: requirement.interface.semantic_hash.to_string(),
        },
        mode: requirement.mode,
        subject: requirement.subject.expect("native target subject"),
        details: requirement
            .details
            .unwrap_or(SemanticHash::from_bytes([0; 32]))
            .to_string(),
        capacity: BudgetDocument::from(requirement.minimum_capacity),
    }
}

fn registry(include_native: bool) -> Registry {
    let mut registry = Registry::hosted_primitives();
    register_deterministic_network_providers(&mut registry).unwrap();
    if include_native {
        install_native_userspace_route_implementation(&mut registry).unwrap();
    }
    registry
}

fn route(plan: &conduit_compile::ExactPlanDocument) -> &conduit_compile::PlanNodeDocument {
    plan.nodes
        .iter()
        .find(|node| node.contract.id == "net/packet/route")
        .expect("route node is planned")
}

#[test]
fn unchanged_network_source_binds_reference_and_observed_native_implementations() {
    let reference_registry = registry(false);
    let reference_host = host("network-reference");
    let reference_profile = InstalledProfile::observe_registry_on_host(
        SOURCE,
        &reference_registry,
        &reference_host,
        &[],
    )
    .unwrap()
    .with_implementation_preference(vec![REFERENCE_ROUTE_IMPLEMENTATION_ID.to_owned()])
    .unwrap();
    let reference_plan = compile_source(SOURCE, &reference_profile.input).unwrap();

    let native_registry = registry(true);
    let mut native_host = host("linux-native-network");
    native_host.capabilities.push(observed_capability(
        native_userspace_route_capability_requirement(),
    ));
    let native_profile =
        InstalledProfile::observe_registry_on_host(SOURCE, &native_registry, &native_host, &[])
            .unwrap()
            .with_implementation_preference(vec![
                NATIVE_USERSPACE_ROUTE_IMPLEMENTATION_ID.to_owned(),
            ])
            .unwrap();
    let native_plan = compile_source(SOURCE, &native_profile.input).unwrap();

    assert_eq!(
        reference_plan.source_semantic_hash,
        native_plan.source_semantic_hash
    );
    assert_eq!(reference_plan.cords, native_plan.cords);
    assert_ne!(reference_plan.identity, native_plan.identity);
    assert_eq!(
        route(&reference_plan).implementation.id,
        REFERENCE_ROUTE_IMPLEMENTATION_ID
    );
    assert_eq!(
        route(&native_plan).implementation.id,
        NATIVE_USERSPACE_ROUTE_IMPLEMENTATION_ID
    );
    assert_eq!(
        route(&reference_plan).contract,
        route(&native_plan).contract
    );
    assert_ne!(
        route(&reference_plan).artifact,
        route(&native_plan).artifact
    );
    assert_ne!(route(&reference_plan).host, route(&native_plan).host);
}

#[test]
fn installation_without_independent_host_capability_cannot_select_native_route() {
    let registry = registry(true);
    let unsupported_host = host("firmware-without-native-route");
    let profile =
        InstalledProfile::observe_registry_on_host(SOURCE, &registry, &unsupported_host, &[])
            .unwrap()
            .with_implementation_preference(vec![
                NATIVE_USERSPACE_ROUTE_IMPLEMENTATION_ID.to_owned(),
            ])
            .unwrap();

    let native_candidate = profile
        .input
        .candidates
        .iter()
        .find(|candidate| candidate.implementation.id == NATIVE_USERSPACE_ROUTE_IMPLEMENTATION_ID)
        .expect("installed native candidate remains inspectable");
    assert_eq!(native_candidate.capabilities.len(), 1);
    assert!(native_candidate.host_report.capabilities.is_empty());

    let plan = compile_source(SOURCE, &profile.input).unwrap();
    assert_eq!(
        route(&plan).implementation.id,
        REFERENCE_ROUTE_IMPLEMENTATION_ID,
        "the exact resolver must fall back to a conforming implementation"
    );
}

#[test]
fn stale_generic_host_snapshot_rejects_before_network_execution() {
    let registry = registry(true);
    let mut stale_host = host("stale-native-network");
    stale_host.capabilities.push(observed_capability(
        native_userspace_route_capability_requirement(),
    ));
    stale_host.current_tick = stale_host.valid_until_tick;
    let profile =
        InstalledProfile::observe_registry_on_host(SOURCE, &registry, &stale_host, &[]).unwrap();
    assert_eq!(
        compile_source(SOURCE, &profile.input).unwrap_err().code(),
        "CND-CMP-008"
    );
}
