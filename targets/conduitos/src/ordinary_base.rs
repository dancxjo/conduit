//! Exact serial provider provenance selected by ordinary Plans.
use crate::{
    identity::hex,
    machine::BaseKind,
    offer::{BaseLifecycle as FixedLifecycle, HostOffer},
    ordinary_plan::PreparationError,
};
use alloc::vec;
use conduit_core::{
    BaseEnforcementClass, BaseLifecycle, BaseProviderAdvertisement, HostAdvertisement,
};

pub(super) const SERIAL_PROVIDER_IMPLEMENTATION: &str = "conduitos/base/serial@1";

pub(super) fn append_serial(
    advertisement: &mut HostAdvertisement,
    fixed: &HostOffer<'_>,
) -> Result<(), PreparationError> {
    let base = fixed
        .bases
        .iter()
        .find(|base| base.kind == BaseKind::Serial)
        .ok_or(PreparationError::OfferMismatch)?;
    if base.lifecycle != FixedLifecycle::Ready {
        return Ok(());
    }
    let capabilities = advertisement
        .capabilities
        .iter()
        .filter(|capability| {
            fixed.capabilities.iter().any(|offered| {
                offered.required_base == BaseKind::Serial
                    && offered.implementation
                        == capability.implementation.implementation_id.as_str()
                    && offered.kind == capability.kind_id.as_str()
            })
        })
        .map(|capability| capability.capability_id.clone())
        .collect();
    let resources = fixed
        .resources
        .iter()
        .enumerate()
        .filter(|(_, resource)| resource.base == BaseKind::Serial)
        .map(|(index, resource)| {
            alloc::format!("conduitos-pool-{index}-{}", resource.base.as_str()).into()
        })
        .collect();
    advertisement.bases.extend(vec![BaseProviderAdvertisement {
        base_id: hex(&base.id).into(),
        provider_instance_id: hex(&base.provider_instance_id).into(),
        provider_generation: base.provider_generation,
        implementation_id: SERIAL_PROVIDER_IMPLEMENTATION.into(),
        mechanism_family: BaseKind::Serial.as_str().into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capability_ids: capabilities,
        resource_pool_ids: resources,
    }]);
    Ok(())
}
