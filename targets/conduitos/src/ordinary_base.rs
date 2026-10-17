//! Exact physical provider provenance selected by ordinary Plans.
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

pub(super) const TIMER_PROVIDER_IMPLEMENTATION: &str = "conduitos/base/timer-with-clock@1";

pub(super) fn append_serial(
    advertisement: &mut HostAdvertisement,
    fixed: &HostOffer<'_>,
) -> Result<(), PreparationError> {
    append_kind(
        advertisement,
        fixed,
        BaseKind::Serial,
        SERIAL_PROVIDER_IMPLEMENTATION,
    )
}

pub(super) fn append_timer(
    advertisement: &mut HostAdvertisement,
    fixed: &HostOffer<'_>,
) -> Result<(), PreparationError> {
    append_kind(
        advertisement,
        fixed,
        BaseKind::Timer,
        TIMER_PROVIDER_IMPLEMENTATION,
    )
}

fn append_kind(
    advertisement: &mut HostAdvertisement,
    fixed: &HostOffer<'_>,
    kind: BaseKind,
    implementation: &'static str,
) -> Result<(), PreparationError> {
    let base = fixed
        .bases
        .iter()
        .find(|base| base.kind == kind)
        .ok_or(PreparationError::OfferMismatch)?;
    if base.lifecycle != FixedLifecycle::Ready {
        return Ok(());
    }
    // One native timer provider owns a particular clock basis. Seal that pair
    // into the selected instance so replacing either provider changes the Plan.
    let provider_instance_id = if kind == BaseKind::Timer {
        timer_clock_provider(fixed)?.provider_instance_id
    } else {
        base.provider_instance_id
    };
    let capabilities = advertisement
        .capabilities
        .iter()
        .filter(|capability| {
            fixed.capabilities.iter().any(|offered| {
                offered.required_base == kind
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
        .filter(|(_, resource)| resource.base == kind)
        .map(|(index, resource)| {
            alloc::format!("conduitos-pool-{index}-{}", resource.base.as_str()).into()
        })
        .collect();
    advertisement.bases.extend(vec![BaseProviderAdvertisement {
        base_id: hex(&base.id).into(),
        provider_instance_id: hex(&provider_instance_id).into(),
        provider_generation: base.provider_generation,
        implementation_id: implementation.into(),
        mechanism_family: kind.as_str().into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capability_ids: capabilities,
        resource_pool_ids: resources,
    }]);
    Ok(())
}

/// Non-authorizing identity of the Root-owned Timer/Clock provider pair.
pub(super) fn timer_clock_provider(
    fixed: &HostOffer<'_>,
) -> Result<crate::offer::BaseProviderBinding, PreparationError> {
    let capability = fixed
        .capabilities
        .iter()
        .find(|capability| {
            capability.implementation == crate::offer::TIME_EVERY_IMPLEMENTATION
                && capability.required_base == BaseKind::Timer
                && capability.secondary_base == Some(BaseKind::Clock)
        })
        .ok_or(PreparationError::OfferMismatch)?;
    let mut timer = fixed
        .capability_provider(capability)
        .map_err(|_| PreparationError::OfferMismatch)?;
    let clock = fixed
        .bases
        .iter()
        .find(|base| base.kind == BaseKind::Clock && base.lifecycle == FixedLifecycle::Ready)
        .ok_or(PreparationError::OfferMismatch)?;
    timer.provider_instance_id = crate::domain_scope_identity::identity(
        b"timer-clock-provider",
        &[
            &timer.base_id,
            &timer.provider_instance_id,
            &timer.provider_generation.to_le_bytes(),
            &clock.id,
            &clock.provider_instance_id,
            &clock.provider_generation.to_le_bytes(),
        ],
    );
    Ok(timer)
}
