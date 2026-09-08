use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn blast_radius_receipts_are_finite_complete_and_secret_free() {
    let source = include_str!("../../../proof/security/acceptance-matrix.json");
    let matrix: Value = serde_json::from_str(source).unwrap();
    assert_eq!(matrix["schema"], "conduit.security-acceptance/v1");
    let receipts = matrix["receipts"].as_array().unwrap();
    let maximum = matrix["maximum_receipts"].as_u64().unwrap() as usize;
    assert!(!receipts.is_empty() && receipts.len() <= maximum);
    let required = [
        "principal",
        "legitimate_scope",
        "attack",
        "boundary",
        "expected",
        "observer",
        "survival",
        "proof_class",
        "command",
    ];
    let mut principals = BTreeSet::new();
    for receipt in receipts {
        for field in required {
            assert!(receipt[field]
                .as_str()
                .is_some_and(|value| !value.is_empty()));
        }
        assert!(principals.insert(receipt["principal"].as_str().unwrap()));
        assert!(receipt["command"]
            .as_str()
            .unwrap()
            .starts_with("cargo xtask "));
    }
    for required_principal in [
        "hostile-wasm-gear",
        "malicious-base-client",
        "compromised-base-provider",
        "compromised-federation-part-b",
        "malicious-ros-client",
        "hostile-conduitos-domain",
        "consequential-effect-proposer",
    ] {
        assert!(principals.contains(required_principal));
    }
    let lowercase = source.to_ascii_lowercase();
    for forbidden in ["bearer_token", "private_key", "issuer_key", "secret_key"] {
        assert!(!lowercase.contains(forbidden));
    }
}
