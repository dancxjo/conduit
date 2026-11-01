//! Closed factory descriptor-volume estimate, not a Rust allocator footprint.
//! Counts two serialized description volumes (registry and projection), known
//! fixed headers, capability/identity arrays, and canonical factory scratch.
//! No JSON tree or serialized byte buffer is materialized for this estimate.
use conduit_core as c;
use std::io::{self, Write};
struct DescriptorVolume(u64);
impl Write for DescriptorVolume {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("expression descriptor volume overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn estimate(
    base: &c::BaseProviderEntry,
    provider: &c::BaseProviderAdvertisement,
    canonical: u64,
) -> Result<u64, String> {
    let mut volume = DescriptorVolume(0);
    serde_json::to_writer(&mut volume, base).map_err(|e| e.to_string())?;
    let vectors = base
        .capabilities
        .capacity()
        .checked_mul(std::mem::size_of::<c::CapabilityOffer>())
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| {
            n.checked_add(
                provider.capability_ids.capacity() * std::mem::size_of::<c::CapabilityId>(),
            )
        })
        .ok_or("expression descriptor vector estimate overflow")?;
    let identities = provider
        .capability_ids
        .iter()
        .map(|id| id.as_str().len())
        .sum::<usize>()
        + provider.base_id.as_str().len()
        + provider.provider_instance_id.as_str().len()
        + provider.implementation_id.as_str().len()
        + provider.mechanism_family.as_str().len();
    let fixed = std::mem::size_of::<c::BaseProviderEntry>()
        + std::mem::size_of::<c::BaseProviderAdvertisement>();
    volume
        .0
        .checked_mul(2)
        .and_then(|n| n.checked_add(vectors as u64))
        .and_then(|n| n.checked_add(identities as u64))
        .and_then(|n| n.checked_add(fixed as u64))
        .and_then(|n| n.checked_add(canonical))
        .ok_or("expression descriptor volume estimate overflow".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counting_writer_matches_existing_serialized_factory_volume() {
        let ty = c::StructuredInfoType::leaf(c::kind_id(c::BOOL_INFO_ID)).unwrap();
        let program = conduit_plot::PortableExpressionProgram {
            input_type: ty.clone(),
            input_maximum_bytes: None,
            output_type: ty.clone(),
            root: conduit_plot::PortableExpressionNode {
                value_type: ty,
                operation: conduit_plot::PortableExpressionOperation::Input,
            },
        };
        let offer = conduit_std_offers::pure_expression_std_offer(&program, c::PortTemporal::Value)
            .unwrap();
        let mut counted = DescriptorVolume(0);
        serde_json::to_writer(&mut counted, &offer).unwrap();
        assert_eq!(
            counted.0 as usize,
            serde_json::to_vec(&offer).unwrap().len()
        );
    }
}
