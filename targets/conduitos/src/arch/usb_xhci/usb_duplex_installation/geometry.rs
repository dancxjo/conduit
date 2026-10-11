//! Exact native metadata and DMA geometry; descriptive facts create no ownership.
use super::super::protocol_installation::{UsbNativeProtocolRefusal, mapped_region};
use crate::usb_base::installation::{UsbInstallationRefusal, UsbNativeIdentity};

pub(super) fn validate_identities(
    identities: [&UsbNativeIdentity; 3],
    epoch: u32,
) -> Result<(), UsbNativeProtocolRefusal> {
    for (index, identity) in identities.iter().enumerate() {
        identity
            .validate()
            .map_err(UsbNativeProtocolRefusal::Installation)?;
        if epoch == 0
            || identity.provider_generation != u64::from(epoch)
            || identity.host_id != identities[0].host_id
            || identity.boot_id != identities[0].boot_id
            || identity.artifact_id != identities[0].artifact_id
            || identities[..index].iter().any(|previous| {
                previous.base_id == identity.base_id
                    || previous.provider_instance_id == identity.provider_instance_id
                    || previous.resource_pool_id == identity.resource_pool_id
            })
        {
            return Err(UsbNativeProtocolRefusal::Installation(
                UsbInstallationRefusal::InvalidIdentity,
            ));
        }
    }
    Ok(())
}
pub(super) fn validate_dma(
    ring_virtual: u64,
    buffer_virtual: u64,
    ring: u64,
    buffer: u64,
    mapping: fn(u64) -> Option<u64>,
) -> Result<(), ()> {
    if ring & 63 != 0
        || ring & 0xffff > 0xfc00
        || buffer & 0xffff > 0xf800
        || !mapped_region(ring_virtual, ring, 1024, mapping)
        || !mapped_region(buffer_virtual, buffer, 2048, mapping)
        || !separate_regions(&[(ring, 1024), (buffer, 2048)])
    {
        return Err(());
    }
    Ok(())
}
pub(super) fn separate_regions(regions: &[(u64, u64)]) -> bool {
    regions.iter().enumerate().all(|(index, (start, bytes))| {
        let Some(end) = start.checked_add(*bytes) else {
            return false;
        };
        *start != 0
            && *bytes != 0
            && regions[..index].iter().all(|(previous, bytes)| {
                previous
                    .checked_add(*bytes)
                    .is_some_and(|previous_end| end <= *previous || previous_end <= *start)
            })
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use conduit_core::ResourceGenerationId;
    fn identity(name: &str) -> UsbNativeIdentity {
        UsbNativeIdentity {
            host_id: "host/product".into(),
            boot_id: "boot/current".into(),
            base_id: format!("base/{name}").into(),
            provider_instance_id: format!("provider/{name}").into(),
            provider_generation: 7,
            resource_pool_id: format!("pool/{name}").into(),
            resource_generation_id: ResourceGenerationId(format!("generation/{name}")),
            envelope_id: format!("envelope/{name}").into(),
            artifact_id: "artifact/product".into(),
        }
    }
    #[test]
    fn duplex_identities_require_one_actual_epoch_and_distinct_native_providers() {
        let control = identity("control");
        let read = identity("read");
        let write = identity("write");
        validate_identities([&control, &read, &write], 7).unwrap();
        let changes: &[fn(&mut UsbNativeIdentity)] = &[
            |i| i.provider_generation = 6,
            |i| i.host_id = "host/other".into(),
            |i| i.boot_id = "boot/stale".into(),
            |i| i.artifact_id = "artifact/foreign".into(),
            |i| i.base_id = "base/control".into(),
            |i| i.provider_instance_id = "provider/read".into(),
            |i| i.resource_pool_id = "pool/control".into(),
        ];
        for change in changes {
            let mut foreign = write.clone();
            change(&mut foreign);
            assert!(validate_identities([&control, &read, &foreign], 7).is_err());
        }
        assert!(validate_identities([&control, &read, &write], 0).is_err());
    }
    #[test]
    fn transport_dma_cannot_alias_another_endpoint_or_control_storage() {
        let native = [
            (0x10000, 8192),
            (0x20000, 1024),
            (0x20800, 2048),
            (0x30000, 1024),
            (0x30800, 2048),
        ];
        assert!(separate_regions(&native));
        for aliased in [
            (0x10000, 1024),
            (0x11000, 2048),
            (0x20000, 1024),
            (0x20800, 2048),
        ] {
            let mut changed = native;
            changed[3] = aliased;
            assert!(!separate_regions(&changed));
        }
        assert!(!separate_regions(&[(u64::MAX - 10, 20)]));
        assert!(!separate_regions(&[(0, 1024)]));
        assert!(separate_regions(&[(0x1000, 1024), (0x1400, 1024)]));
    }
    #[test]
    fn each_real_dma_region_requires_exact_coherent_mapping_and_geometry() {
        fn coherent(address: u64) -> Option<u64> {
            address.checked_add(0x10000)
        }
        assert!(validate_dma(0x1000, 0x1800, 0x11000, 0x11800, coherent).is_ok());
        assert!(validate_dma(0x1000, 0x1800, 0x11040, 0x11800, coherent).is_err());
        assert!(validate_dma(0x1000, 0x1800, 0x11000, 0x11000, coherent).is_err());
        assert!(validate_dma(0x1000, 0x1800, 0x11000, 0x1ff00, coherent).is_err());
    }
}
