//! Compact current provider facts alongside separately fabricated offer images.
use super::*;
use alloc::{string::String, vec::Vec};
use core::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I2cImageProjectionRefusal {
    Context,
    Capacity,
    Allocation,
}

/// Ordinary Core metadata only. The caller separately funds containing vectors,
/// signed publication and immutable image references. No effect handle is issued.
pub struct I2cImageProjection {
    pub base: BaseProviderAdvertisement,
    pub resource: ResourceOffer,
    pub owned_bytes: usize,
}
impl<P: I2cProvider> ReadyI2cBase<P> {
    /// Project only this actually retained Ready provider. The preparing Host
    /// resolves the separately published image; this MCU retains no rich offer.
    pub fn project_for_images(
        &self,
        actual: &HostAdvertisement,
        maximum_bytes: usize,
    ) -> Result<I2cImageProjection, I2cImageProjectionRefusal> {
        use I2cImageProjectionRefusal as E;
        let id = &self.identity;
        if actual.host_id != id.host_id
            || actual.boot_id != id.boot_id
            || actual.bases.iter().any(|b| {
                b.base_id == id.base_id || b.provider_instance_id == id.provider_instance_id
            })
            || actual
                .resources
                .iter()
                .any(|r| r.pool_id == id.resource_pool_id)
            || actual
                .capabilities
                .iter()
                .any(|c| c.capability_id.as_str() == I2C_CAPABILITY)
        {
            return Err(E::Context);
        }
        let fields = [
            id.base_id.as_str(),
            id.provider_instance_id.as_str(),
            id.base_implementation_id.as_str(),
            I2C_ATTACHMENT_CLASS,
            I2C_CAPABILITY,
            id.resource_pool_id.as_str(),
            id.resource_pool_id.as_str(),
            I2C_ATTACHMENT_CLASS,
        ];
        let vector_bytes = size_of::<CapabilityId>()
            .checked_add(size_of::<ResourcePoolId>())
            .ok_or(E::Capacity)?;
        let required = fields
            .iter()
            .try_fold(vector_bytes, |n, s| n.checked_add(s.len()))
            .ok_or(E::Capacity)?;
        if required > maximum_bytes {
            return Err(E::Capacity);
        }
        let mut remaining = maximum_bytes;
        let mut copy = |text: &str| -> Result<String, E> {
            let mut value = String::new();
            value
                .try_reserve_exact(text.len())
                .map_err(|_| E::Allocation)?;
            remaining = remaining.checked_sub(value.capacity()).ok_or(E::Capacity)?;
            value.push_str(text);
            Ok(value)
        };
        let base_id = copy(fields[0])?.into();
        let provider_instance_id = copy(fields[1])?.into();
        let implementation_id = copy(fields[2])?.into();
        let mechanism_family = copy(fields[3])?.into();
        let capability = copy(fields[4])?.into();
        let resource_id = copy(fields[5])?.into();
        let pool_id = copy(fields[6])?.into();
        let class_id = copy(fields[7])?.into();
        let mut capability_ids = Vec::new();
        capability_ids
            .try_reserve_exact(1)
            .map_err(|_| E::Allocation)?;
        let mut resource_pool_ids = Vec::new();
        resource_pool_ids
            .try_reserve_exact(1)
            .map_err(|_| E::Allocation)?;
        remaining = remaining
            .checked_sub(
                capability_ids
                    .capacity()
                    .checked_mul(size_of::<CapabilityId>())
                    .ok_or(E::Capacity)?,
            )
            .and_then(|n| {
                n.checked_sub(
                    resource_pool_ids
                        .capacity()
                        .checked_mul(size_of::<ResourcePoolId>())?,
                )
            })
            .ok_or(E::Capacity)?;
        capability_ids.push(capability);
        resource_pool_ids.push(resource_id);
        Ok(I2cImageProjection {
            base: BaseProviderAdvertisement {
                base_id,
                provider_instance_id,
                provider_generation: id.provider_generation,
                implementation_id,
                mechanism_family,
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capability_ids,
                resource_pool_ids,
            },
            resource: ResourceOffer {
                pool_id,
                class_id,
                capacity_units: 1,
                compute: None,
                content: None,
            },
            owned_bytes: maximum_bytes - remaining,
        })
    }
}
