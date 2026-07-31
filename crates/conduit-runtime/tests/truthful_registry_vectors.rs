mod support;

use conduit_core::{PinnedDescriptor, SemanticHash};
use conduit_panel::{Node, parse};
use conduit_runtime::{
    AvailabilityState, Handler, Registry, RunIo, RuntimeError, SourceContractCatalog, Value,
    file_read_contract,
};

struct DummyHandler;
const FIXTURE: &str = include_str!("../../../conformance/c5/registry-availability.json");

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
        "std/literal",
        "std/format-values/literal",
        "std/text/format",
        "std/text/lines",
        "std/text/join",
        "io/stdin",
        "text/uppercase",
        "std/data/encode-utf8",
        "std/data/decode-utf8",
        "std/data/frame-length-u32be",
        "std/data/deframe-length-u32be",
        "std/data/validate-closed-record",
        "io/stdout",
        "io/stderr",
        "display/text",
        "supervision/supervisor",
        "flow/identity",
        "conduit.std/tee",
        "conduit.std/merge",
        "conduit.std/zip",
        "conduit.std/gate",
        "conduit.std/select",
        "flow/fallback",
        "time/delay",
        "time/timeout",
        "time/debounce",
        "time/throttle",
        "flow/take",
        "flow/skip",
        "flow/filter",
        "test/probe",
        "observe/log",
        "test/assertion",
        "test/record",
        "test/replay",
        "test/fault-source",
        "fs/read",
        "fs/write",
        "fs/watch",
        "storage/blob/literal",
        "storage/cache/put",
        "storage/cache/get",
        "storage/cache/remove",
        "conduit.host/process/exec",
        "device/gpio/pin",
        "device/serial/port",
        "state/cell",
        "state/counter",
        "state/deduplicate",
        "state/cache",
        "supervision/retry",
        "supervision/circuit-breaker",
        "supervision/health-gate",
        "net/wifi/join",
        "net/wifi/access-point",
        "net/interface",
        "net/tcp/socket",
        "net/udp/socket",
        "net/dns/resolve",
        "net/http/serve",
    ] {
        let availability = registry.node_availability(kind);
        assert_eq!(
            availability.state,
            AvailabilityState::ContractOnly,
            "{kind}"
        );
        assert_eq!(availability.reason_code, "CND-AVL-001");
        let panel = parse(&format!("panel 0\nnode node : {kind}\n")).unwrap();
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
fn definitions_do_not_claim_host_support_and_displaced_names_are_absent() {
    let registry = Registry::default();

    let http = registry.node_availability("net/http/serve");
    assert_eq!(http.contract_id, "net/http/serve");
    assert_eq!(http.state, AvailabilityState::ContractOnly);
    let http_contract = registry
        .node_contract("net/http/serve")
        .expect("HTTP serving meaning is defined");
    assert_eq!(http_contract.inputs[0].value_type.id, "net/http/response");
    assert_eq!(http_contract.outputs[0].value_type.id, "net/http/request");
    assert_eq!(
        registry.node_availability("conduit.std/http-server").state,
        AvailabilityState::Unsupported
    );

    let integer = registry
        .type_reference("std/integer")
        .expect("mathematical integer is defined");
    assert_eq!(integer.id, "std/integer");
    assert!(
        registry
            .type_registry()
            .describe(conduit_std::standard_type_reference("std/integer").unwrap())
            .is_some()
    );
    assert!(registry.type_reference("conduit/integer").is_none());
}

#[test]
fn compatibility_demo_runs_only_proven_finite_handlers_without_claiming_availability() {
    let registry = Registry::compatibility_demo();
    for kind in [
        "std/literal",
        "std/format-values/literal",
        "std/text/format",
        "std/text/lines",
        "std/text/join",
        "io/stdin",
        "text/uppercase",
        "io/stdout",
        "io/stderr",
        "supervision/supervisor",
        "flow/identity",
        "conduit.std/tee",
        "conduit.std/merge",
        "flow/fallback",
    ] {
        assert_eq!(
            registry.node_availability(kind).state,
            AvailabilityState::ContractOnly
        );
    }
    let panel = parse(
        "panel 0\n\
         node source : std/literal { value = \"fixture\" }\n\
         node upper : text/uppercase\n\
         node encoded : std/data/encode-utf8 { codec = ref(\"conduit.codec/utf-8\") codec_schema_version = 0 codec_hash = bytes(\"f219297cb276bc91eccddb346a8b21e7edd4414b8844014108513747ae11bf53\") maximum_input_bytes = 4096 maximum_output_bytes = 4096 }\n\
         node sink : io/stdout\n\
         cord source.value -> upper.text\n\
         cord upper.text -> encoded.text\n\
         cord encoded.bytes -> sink.bytes\n",
    )
    .unwrap();
    registry
        .resolve(&panel)
        .expect("proven compatibility pipeline resolves");
}

#[test]
fn hosted_primitive_registry_couples_callbacks_to_installed_artifacts() {
    let registry = Registry::hosted_primitives();
    let installed = Registry::installed_hosted_providers();
    let registered = registry.installed_providers();
    let installed_identities = installed
        .iter()
        .map(|provider| (provider.contract.id.as_str(), provider.manifest.id.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    let registered_identities = registered
        .iter()
        .map(|provider| (provider.contract.id.as_str(), provider.manifest.id.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(registered_identities, installed_identities);
    assert!(
        installed
            .iter()
            .any(|provider| provider.contract.id.as_str() == "display/text")
    );
    for provider in installed {
        let availability = registry.node_availability(provider.contract.id.as_str());
        assert_eq!(availability.state, AvailabilityState::ProviderAvailable);
        assert_eq!(
            availability.implementation_id.as_deref(),
            Some(provider.manifest.id.as_str())
        );
        assert_eq!(
            provider.manifest.artifacts[0].digest,
            provider.artifact.digest
        );
    }
}

#[test]
fn exact_core_manifest_installation_is_provider_available_but_not_host_resolvable() {
    let fixture = support::provider(file_read_contract(), "test/file-read-native");
    let mut registry = Registry::default();
    registry
        .register_executable_provider(
            file_read_contract(),
            fixture.manifest,
            fixture.artifacts,
            || Box::new(DummyHandler),
            |_| Ok(()),
        )
        .unwrap();
    let availability = registry.node_availability("fs/read");
    assert_eq!(availability.state, AvailabilityState::ProviderAvailable);
    assert_eq!(availability.reason_code, "CND-AVL-002");
    assert_eq!(
        availability.implementation_id.as_deref(),
        Some("test/file-read-native")
    );
    assert_eq!(availability.host_id, None);
    assert_eq!(availability.rejection_reasons, vec!["CND-RES-025"]);

    let panel = parse("panel 0\nnode file : fs/read\n").unwrap();
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
        .node_schema("std/literal")
        .expect("literal schema");
    let fixture = support::provider_with_contract(
        file_read_contract(),
        PinnedDescriptor {
            id: conduit_core::Id("std/literal"),
            schema_version: 0,
            semantic_hash: literal.semantic_hash(),
        },
        "test/impersonator",
    );
    let mut registry = Registry::default();
    assert_eq!(
        registry
            .register_executable_provider(
                file_read_contract(),
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
        file_read_contract(),
        PinnedDescriptor {
            id: file_read_contract().id,
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([77; 32]),
        },
        "test/wrong-contract-hash",
    );
    let mut registry = Registry::default();
    assert_eq!(
        registry
            .register_executable_provider(
                file_read_contract(),
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
    let fixture = support::provider(file_read_contract(), "test/missing-artifact");
    let mut registry = Registry::default();
    assert_eq!(
        registry
            .register_executable_provider(
                file_read_contract(),
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
    for discarded in [
        "conduit.std/literal",
        "flow/tee",
        "flow/merge",
        "flow/zip",
        "flow/gate",
        "flow/select",
        "process/run",
        "process/stream",
    ] {
        let availability = registry.node_availability(discarded);
        assert_eq!(
            availability.state,
            AvailabilityState::Unsupported,
            "{discarded}"
        );
        assert_eq!(availability.reason_code, "CND-AVL-006", "{discarded}");
        assert_eq!(
            availability.rejection_reasons,
            vec!["CND-RES-001"],
            "{discarded}"
        );
        let panel = parse(&format!("panel 0\nnode legacy : {discarded}\n")).unwrap();
        assert_eq!(
            registry
                .resolve(&panel)
                .expect_err("discarded ID must fail")
                .code,
            "CND-IMP-001",
            "{discarded}"
        );
    }
}

#[test]
fn patchbay_receives_registry_facts_without_node_name_inference() {
    let workspace = conduit_patchbay::Workspace::new(
        "doc-1",
        "panel 0\nnode greeting : std/literal { value = \"hello\" }\n",
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
