use std::collections::BTreeSet;

use conduit_net::{NETWORK_CONTRACTS, NETWORK_VALUE_TYPES, STANDING_NETWORK_CONTRACTS};

#[test]
fn issue_270_inventory_names_every_required_type_proof_and_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c4/standing-network.json"
    ))
    .unwrap();
    assert_eq!(fixture["schema_version"], 0);
    assert_eq!(fixture["issue"], 270);

    let types = fixture["types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(types.len(), NETWORK_VALUE_TYPES.len());
    for value_type in NETWORK_VALUE_TYPES {
        assert!(types.contains(value_type.contract_id.as_str()));
    }

    let contracts = fixture["standing_contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts.len(),
        STANDING_NETWORK_CONTRACTS.len() + NETWORK_CONTRACTS.len()
    );
    for contract in STANDING_NETWORK_CONTRACTS
        .into_iter()
        .chain(NETWORK_CONTRACTS)
    {
        assert!(contracts.contains(contract.id.as_str()));
    }

    let proofs = fixture["proofs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "isolated-local-network",
        "client-path",
        "packet-path",
        "retained-state",
        "continuous-service",
        "observation",
        "tour-project",
    ] {
        assert!(proofs.contains(required), "missing proof {required}");
    }

    let cases = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "link-absent",
        "link-present",
        "link-flapping",
        "ap-sta-concurrency-impossible",
        "association-without-address",
        "dhcp-zero-last-exhausted",
        "route-add-remove-replace",
        "no-route",
        "zero-delay-route-loop",
        "ttl-hop-exhaustion",
        "mtu-fragmentation-policy",
        "forwarding-denied",
        "frame-packet-queue-pressure",
        "udp-loss-duplicate-reorder",
        "tcp-partial-io-half-close-reset",
        "listener-session-exhaustion",
        "name-success-nxdomain-stale",
        "mdns-conflict",
        "discovery-expiry",
        "drain-in-flight",
        "provider-loss",
        "host-reboot",
        "stale-grant-lease-observation",
        "capture-bound-redaction",
        "watch-attached-detached",
        "deterministic-supported",
        "linux-explicit-install",
        "pico-supported-subset",
        "pico-unsupported-route",
        "forged-source-authority",
        "editing-does-not-start",
        "independent-effects-contract-only",
        "correlation-delivery-skew",
    ] {
        assert!(cases.contains(required), "missing fixture {required}");
    }
}

#[test]
fn provider_matrix_is_plural_observation_gated_and_never_claims_unsupported_pico_routing() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c4/standing-network.json"
    ))
    .unwrap();
    let providers = fixture["provider_matrix"].as_array().unwrap();
    assert!(providers.iter().any(|entry| {
        entry["provider"] == "conduit.net/packet-router-reference"
            && entry["contract"] == "net/packet/route"
            && entry["state"] == "supported"
    }));
    assert!(providers.iter().any(|entry| {
        entry["provider"] == "conduit.net/native-userspace-route-table"
            && entry["contract"] == "net/packet/route"
            && entry["state"] == "requires-fresh-host-capability"
    }));
    assert!(providers.iter().any(|entry| {
        entry["provider"] == "netherwick/pico-w"
            && entry["contract"] == "net/packet/route"
            && entry["state"] == "unsupported"
    }));
}
