//! Closed factory descriptor estimate. Counts serialized/tree staging and
//! the two owned factory descriptor copies (registry and projection).
//! This post-construction estimate is not an allocator admission or RSS bound.
use conduit_core as c;
pub(super) fn estimate(
    base: &c::BaseProviderEntry,
    provider: &c::BaseProviderAdvertisement,
    canonical: u64,
) -> Result<u64, String> {
    let wire = serde_json::to_vec(base).map_err(|e| e.to_string())?;
    let tree: serde_json::Value = serde_json::from_slice(&wire).map_err(|e| e.to_string())?;
    let descriptor = json_backing(&tree)?
        .checked_add(std::mem::size_of::<c::BaseProviderEntry>())
        .ok_or("expression descriptor overflow")?;
    let provider_ids = provider
        .capability_ids
        .iter()
        .map(|id| id.as_str().len())
        .sum::<usize>()
        + provider.capability_ids.capacity() * std::mem::size_of::<c::CapabilityId>()
        + provider.base_id.as_str().len()
        + provider.provider_instance_id.as_str().len()
        + provider.implementation_id.as_str().len()
        + provider.mechanism_family.as_str().len()
        + std::mem::size_of::<c::BaseProviderAdvertisement>();
    descriptor
        .checked_mul(2)
        .and_then(|n| n.checked_add(wire.capacity()))
        .and_then(|n| n.checked_add(provider_ids))
        .and_then(|n| (n as u64).checked_add(canonical))
        .ok_or("expression descriptor estimate overflow".into())
}
fn json_backing(value: &serde_json::Value) -> Result<usize, String> {
    let own = std::mem::size_of::<serde_json::Value>();
    let extra = match value {
        serde_json::Value::String(s) => s.capacity(),
        serde_json::Value::Array(items) => items.iter().try_fold(
            items.capacity() * std::mem::size_of::<serde_json::Value>(),
            |n, v| {
                n.checked_add(json_backing(v)?)
                    .ok_or_else(|| "descriptor array footprint overflow".to_string())
            },
        )?,
        serde_json::Value::Object(items) => items.iter().try_fold(
            items.len() * std::mem::size_of::<(String, serde_json::Value)>(),
            |n, (k, v)| {
                n.checked_add(k.capacity())
                    .and_then(|n| n.checked_add(json_backing(v).ok()?))
                    .ok_or_else(|| "descriptor object footprint overflow".to_string())
            },
        )?,
        _ => 0,
    };
    own.checked_add(extra)
        .ok_or("descriptor footprint overflow".into())
}
