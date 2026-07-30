mod support;

use conduit_core::{PinnedDescriptor, SemanticHash};
use conduit_panel::{Node, parse};
use conduit_runtime::{
    AvailabilityState, FILE_READ_CONTRACT, Handler, Registry, RunIo, RuntimeError,
    SourceContractCatalog, Value,
};

struct DummyHandler;
const FIXTURE: &str = include_str!("../../../conformance/c5/registry-availability-v1.json");

impl Handler for DummyHandler {
    fn run(
        &mut self,
        _: &Node,
        _: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        Ok(Vec::new())
    }
}

#[test]
fn registry_availability_fixture_names_every_required_boundary() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let ids = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    for required in [
        "default-contract-is-contract-only",
        "compatibility-demo-is-not-provider-availability",
        "exact-manifest-and-artifact-install-provider",
        "resolved-placement-establishes-host-resolvability",
        "exact-plan-establishes-bound-state",
        "typed-run-event-establishes-running-state",
        "unknown-contract-is-unsupported",
        "missing-implementation-artifact-rejected",
        "incompatible-contract-hash-rejected",
        "stale-host-report-rejected",
        "insufficient-host-budget-rejected",
        "missing-or-denied-grant-rejected",
        "same-contract-callback-proves-no-behavior",
        "noncanonical-standard-id-is-not-aliased",
    ] {
        assert!(ids.contains(required), "fixture covers {required}");
    }
}

#[test]
fn default_registry_publishes_contracts_without_installing_callbacks() {
    let registry = Registry::default();
    for kind in [
        "conduit.std/literal",
        "conduit.std/stdin",
        "conduit.std/uppercase",
        "conduit.std/stdout",
        "conduit.std/stderr",
        "conduit.std/supervisor",
        "conduit.std/pass-through",
        "conduit.std/tee",
        "conduit.std/merge",
        "conduit.std/fallback",
        "conduit.std/delay",
        "conduit.std/debounce",
        "conduit.std/throttle",
        "conduit.std/take",
        "conduit.std/skip",
        "conduit.std/filter",
        "conduit.std/probe",
        "conduit.std/log",
        "conduit.std/assert",
        "conduit.std/record",
        "conduit.std/replay",
        "conduit.std/fault-source",
        "conduit.std/file-read",
        "conduit.std/file-write",
        "conduit.std/blob-store",
        "conduit.std/kv-store",
        "conduit.std/process-spawn",
        "conduit.std/gpio-pin",
        "conduit.std/serial-port",
        "conduit.std/cell",
        "conduit.std/counter",
        "conduit.std/deduplicate",
        "conduit.std/cache",
        "conduit.std/circuit-breaker",
        "conduit.std/health-gate",
        "conduit.std/backoff",
        "conduit.std/wifi-station",
        "conduit.std/wifi-ap",
        "conduit.std/network-interface",
        "conduit.std/tcp-socket",
        "conduit.std/udp-socket",
        "conduit.std/dns-resolver",
    ] {
        let availability = registry.node_availability(kind);
        assert_eq!(
            availability.state,
            AvailabilityState::ContractOnly,
            "{kind}"
        );
        assert_eq!(availability.reason_code, "CND-AVL-001");
        let panel = parse(&format!("panel 1\nnode node : {kind}\n")).unwrap();
        assert_eq!(
            registry
                .resolve(&panel)
                .expect_err("default registry must not execute contracts")
                .code,
            "CND-IMP-001"
        );
    }
}

#[test]
fn compatibility_demo_runs_only_proven_finite_handlers_without_claiming_availability() {
    let registry = Registry::compatibility_demo();
    for kind in [
        "conduit.std/literal",
        "conduit.std/stdin",
        "conduit.std/uppercase",
        "conduit.std/stdout",
        "conduit.std/stderr",
        "conduit.std/supervisor",
        "conduit.std/pass-through",
        "conduit.std/tee",
        "conduit.std/merge",
        "conduit.std/fallback",
    ] {
        assert_eq!(
            registry.node_availability(kind).state,
            AvailabilityState::ContractOnly
        );
    }
    let panel = parse(
        "panel 1\n\
         node source : conduit.std/literal { value = \"fixture\" }\n\
         node upper : conduit.std/uppercase\n\
         node sink : conduit.std/stdout\n\
         cord source.out -> upper.in\n\
         cord upper.out -> sink.in\n",
    )
    .unwrap();
    registry
        .resolve(&panel)
        .expect("proven compatibility pipeline resolves");
}

#[test]
fn exact_core_manifest_installation_is_provider_available_but_not_host_resolvable() {
    let fixture = support::provider(&FILE_READ_CONTRACT, "test/file-read-native");
    let mut registry = Registry::default();
    registry
        .register_executable_provider(
            &FILE_READ_CONTRACT,
            fixture.manifest,
            fixture.artifacts,
            || Box::new(DummyHandler),
            |_| Ok(()),
        )
        .unwrap();
    let availability = registry.node_availability("conduit.std/file-read");
    assert_eq!(availability.state, AvailabilityState::ProviderAvailable);
    assert_eq!(availability.reason_code, "CND-AVL-002");
    assert_eq!(
        availability.implementation_id.as_deref(),
        Some("test/file-read-native")
    );
    assert_eq!(availability.host_id, None);
    assert_eq!(availability.rejection_reasons, vec!["CND-RES-025"]);

    let panel = parse("panel 1\nnode file : conduit.std/file-read\n").unwrap();
    assert_eq!(
        registry
            .resolve(&panel)
            .expect_err("installed provider cannot enter compatibility execution")
            .code,
        "CND-IMP-001"
    );
}

#[test]
fn cross_contract_semantic_impersonation_is_rejected() {
    let literal = Registry::default()
        .node_schema("conduit.std/literal")
        .expect("literal schema");
    let fixture = support::provider_with_contract(
        &FILE_READ_CONTRACT,
        PinnedDescriptor {
            id: conduit_core::Id("conduit.std/literal"),
            schema_version: 1,
            semantic_hash: literal.semantic_hash(),
        },
        "test/impersonator",
    );
    let mut registry = Registry::default();
    assert_eq!(
        registry
            .register_executable_provider(
                &FILE_READ_CONTRACT,
                fixture.manifest,
                fixture.artifacts,
                || Box::new(DummyHandler),
                |_| Ok(()),
            )
            .expect_err("cross-contract provider must fail")
            .code,
        "CND-REG-004"
    );
}

#[test]
fn incompatible_contract_hash_is_rejected() {
    let fixture = support::provider_with_contract(
        &FILE_READ_CONTRACT,
        PinnedDescriptor {
            id: FILE_READ_CONTRACT.id,
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([77; 32]),
        },
        "test/wrong-contract-hash",
    );
    let mut registry = Registry::default();
    assert_eq!(
        registry
            .register_executable_provider(
                &FILE_READ_CONTRACT,
                fixture.manifest,
                fixture.artifacts,
                || Box::new(DummyHandler),
                |_| Ok(()),
            )
            .expect_err("wrong semantic hash must fail")
            .code,
        "CND-REG-005"
    );
}

#[test]
fn missing_exact_artifact_is_rejected() {
    let fixture = support::provider(&FILE_READ_CONTRACT, "test/missing-artifact");
    let mut registry = Registry::default();
    assert_eq!(
        registry
            .register_executable_provider(
                &FILE_READ_CONTRACT,
                fixture.manifest,
                &[],
                || Box::new(DummyHandler),
                |_| Ok(()),
            )
            .expect_err("missing exact artifact must fail")
            .code,
        "CND-REG-008"
    );
}

#[test]
fn discarded_standard_id_is_unsupported_and_never_aliased() {
    let registry = Registry::default();
    let availability = registry.node_availability("conduit/literal");
    assert_eq!(availability.state, AvailabilityState::Unsupported);
    assert_eq!(availability.reason_code, "CND-AVL-006");
    assert_eq!(availability.rejection_reasons, vec!["CND-RES-001"]);
    let panel = parse("panel 1\nnode legacy : conduit/literal\n").unwrap();
    assert_eq!(
        registry
            .resolve(&panel)
            .expect_err("discarded ID must fail")
            .code,
        "CND-IMP-001"
    );
}

#[test]
fn patchbay_receives_registry_facts_without_node_name_inference() {
    let workspace = conduit_patchbay::Workspace::new(
        "doc-1",
        "panel 1\nnode greeting : conduit.std/literal { value = \"hello\" }\n",
    )
    .unwrap();
    let registry = Registry::default();
    let snapshot = workspace.semantic_with_lookup(|kind| {
        let availability = registry.node_availability(kind);
        conduit_patchbay::NodeAvailabilityProjection {
            contract_id: availability.contract_id,
            availability_state: availability.state.as_str().to_owned(),
            reason_code: availability.reason_code,
            implementation_id: availability.implementation_id,
            host_id: availability.host_id,
            rejection_reasons: availability.rejection_reasons,
        }
    });
    assert_eq!(snapshot.availabilities.len(), 1);
    assert_eq!(
        snapshot.availabilities[0].availability_state,
        "contract-only"
    );
}
