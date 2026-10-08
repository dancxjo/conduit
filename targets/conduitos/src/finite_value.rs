//! Native Host offers for shared allocation-prepared finite value owners.
//! These preserve semantic Fore identity; no Source count or EOF policy is chosen.
use alloc::{format, vec::Vec};
use conduit_core::*;
pub const VALUE_REPEAT_IMPLEMENTATION: &str = "conduitos/value-repeat-finite@1";
pub const FLOW_EXACTLY_ONE_IMPLEMENTATION: &str = "conduitos/flow-exactly-one@1";
fn offer(
    kind: Kind,
    schema: &StructuredInfoType,
    implementation: &'static str,
    profile: &'static str,
    artifact: &'static str,
) -> Result<CapabilityOffer, &'static str> {
    let mut identity = schema
        .canonical_bytes()
        .map_err(|_| "invalid finite schema")?;
    let fingerprint = compute_checked_front_fingerprint(&kind.checked_front());
    identity.extend_from_slice(fingerprint.as_bytes());
    let digest = semantic_digest("conduitos/finite-value-profile@1", &identity);
    let mut suffix = alloc::string::String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write;
        write!(&mut suffix, "{byte:02x}").map_err(|_| "profile identity formatting failed")?;
    }
    let capability_id = format!("conduitos/{}/{}", kind.kind_id.as_str(), suffix);
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: capability_id.into(),
            execution_profile_id: profile.into(),
            implementation_id: implementation.into(),
            artifact_id: artifact.into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}
pub fn value_repeat_offer(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<CapabilityOffer, &'static str> {
    offer(
        conduit_semantic_catalog::value_repeat_semantic_contract(value, schema)?,
        schema,
        VALUE_REPEAT_IMPLEMENTATION,
        "conduitos/value-repeat-prepared@1",
        "conduit-data/value-repeat-finite@1",
    )
}
pub fn value_repeat_capacity2_offer(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<CapabilityOffer, &'static str> {
    offer(
        conduit_semantic_catalog::value_repeat_capacity2_semantic_contract(value, schema)?,
        schema,
        VALUE_REPEAT_IMPLEMENTATION,
        "conduitos/value-repeat-prepared-capacity2@1",
        "conduit-data/value-repeat-finite@1",
    )
}
pub fn flow_exactly_one_offer(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<CapabilityOffer, &'static str> {
    offer(
        conduit_semantic_catalog::flow_exactly_one_semantic_contract(value, schema)?,
        schema,
        FLOW_EXACTLY_ONE_IMPLEMENTATION,
        "conduitos/flow-exactly-one-prepared@1",
        "conduit-data/flow-exactly-one@1",
    )
}
