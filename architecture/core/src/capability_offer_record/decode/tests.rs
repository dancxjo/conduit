use super::*;
use alloc::{format, vec::Vec};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LegacyOffer {
    #[serde(default)]
    startup_parameters: Vec<FrontStartupParameter>,
    #[serde(default)]
    shorthand: Option<(PortId, PortId)>,
    capability_id: CapabilityId,
    kind_id: KindId,
    kind_contract_revision: KindIdentity,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    semantic_contract: KindSemanticContract,
    #[serde(flatten)]
    implementation: ImplementationOffer,
    /// Exact keep-duration support of this Back. Absence means that the Back
    /// makes no retained-State promise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state_retention: Option<StateRetentionSupport>,
    /// Exact finite domain-owned realization facts. Empty means undeclared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    realization_properties: Vec<StructuredConfigurationValue>,
    host_calls: Vec<HostCallRequirement>,
    resource_requirements: Vec<ResourceRequirement>,
    authority_requirements: Vec<AuthorityRequirement>,
    limits: CapabilityLimits,
}

const OFFER: &str = r#"{"capability_id":"sensor","kind_id":"environment/sample","kind_contract_revision":"v1","inputs":[],"outputs":[],"semantic_contract":{"configuration":[],"laws":[]},"execution_profile_id":"native","implementation_id":"bme280","artifact_id":"firmware","host_calls":[],"resource_requirements":[],"authority_requirements":[],"limits":{"max_active_instances":1,"max_queue_items":1,"max_queue_bytes":64}}"#;
fn agrees(json: &str) {
    let old = serde_json::from_str::<LegacyOffer>(json);
    let new = serde_json::from_str::<CapabilityOffer>(json);
    assert_eq!(old.is_ok(), new.is_ok(), "acceptance differs: {json}");
    if let (Ok(old), Ok(new)) = (old, new) {
        assert_eq!(
            serde_json::to_value(old).unwrap(),
            serde_json::to_value(new).unwrap()
        );
    }
}
#[test]
fn direct_offer_fields_preserve_defaults_unknowns_and_required_fields() {
    agrees(OFFER);
    let original: serde_json::Value = serde_json::from_str(OFFER).unwrap();
    for key in original.as_object().unwrap().keys() {
        let mut missing = original.clone();
        missing.as_object_mut().unwrap().remove(key);
        agrees(&serde_json::to_string(&missing).unwrap());
    }
    let mut extended = original;
    extended["unknown"] = serde_json::json!({"nested":[{"ignored":"value"}]});
    extended["state_retention"] = serde_json::Value::Null;
    extended["startup_parameters"] = serde_json::json!([]);
    extended["realization_properties"] = serde_json::json!([]);
    agrees(&serde_json::to_string(&extended).unwrap());
    agrees(&OFFER.replace("bme280", r"bme\u0032\u0038\u0030"));
}
#[test]
fn direct_offer_fields_reject_duplicate_keys_and_sequence_forms() {
    for (key, value) in [
        ("capability_id", "sensor"),
        ("implementation_id", "bme280"),
        ("artifact_id", "firmware"),
        ("execution_profile_id", "native"),
    ] {
        let duplicate = format!("{{\"{key}\":\"{value}\",{}", &OFFER[1..]);
        assert!(serde_json::from_str::<CapabilityOffer>(&duplicate).is_err());
        agrees(&duplicate);
    }
    for input in ["[]", "[null]", "null", "false", "0", "\"offer\""] {
        assert!(serde_json::from_str::<CapabilityOffer>(input).is_err());
        agrees(input);
    }
}
