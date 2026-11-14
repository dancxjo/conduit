//! Idle Root publication of actual equipment, using the ordinary image protocol.
use crate::{config, i2c_installation};
use alloc::{string::String, vec::Vec};
use conduit_core as c;
use core::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Context,
    Capacity,
    Allocation,
    Native(i2c_installation::Refusal),
    Projection,
}

/// Prepare the next compact generation without mutating the current owner.
/// Root must refuse replacement while any reservation or compute assignment is
/// retained. `publish_native` selects observation, not readiness: preparation
/// must actually acquire the initialized controller and cannot touch the bus.
pub fn prepare(
    actual: &c::HostAdvertisement,
    publish_native: bool,
    maximum_bytes: usize,
) -> Result<c::HostAdvertisement, Refusal> {
    let generation = actual
        .offer_generation
        .0
        .checked_add(1)
        .ok_or(Refusal::Context)?;
    let resident = actual
        .resources
        .iter()
        .find(|r| r.pool_id.as_str() == config::RESIDENT_WORK_POOL)
        .ok_or(Refusal::Context)?;
    if actual.protocol_version != c::PROTOCOL_VERSION
        || actual.declared_name.is_some()
        || actual.profile.as_str() != config::CONFIGURATION_ID
        || actual.bases.len() > 1
        || actual.resources.len() > 2
        || !actual.capabilities.is_empty()
        || !actual.planner_capabilities.is_empty()
        || resident.class_id.as_str() != c::COOPERATIVE_STEP_WORK_CLASS_ID
        || resident.capacity_units != config::RESIDENT_WORK_UNITS
        || resident.compute.is_some()
        || resident.content.is_some()
        || (publish_native && config::I2C_OFFER_IMAGE.is_none())
    {
        return Err(Refusal::Context);
    }
    // This Root owns only its private work pool and the selected native bus.
    // Never rebuild a context by silently dropping an unfamiliar provider.
    if actual
        .resources
        .iter()
        .filter(|r| r.pool_id == resident.pool_id)
        .count()
        != 1
    {
        return Err(Refusal::Context);
    }
    if publish_native {
        if !actual.bases.is_empty() || actual.resources.len() != 1 {
            return Err(Refusal::Context);
        }
    } else {
        let [base] = actual.bases.as_slice() else {
            return Err(Refusal::Context);
        };
        let mut others = actual
            .resources
            .iter()
            .filter(|r| r.pool_id != resident.pool_id);
        let other = others.next().ok_or(Refusal::Context)?;
        if others.next().is_some()
            || base.capability_ids.len() != 1
            || base.capability_ids[0].as_str()
                != conduit_i2c_transaction::installation::I2C_IMPLEMENTATION
            || base.mechanism_family.as_str()
                != conduit_i2c_transaction::installation::I2C_ATTACHMENT_CLASS
            || base.resource_pool_ids.as_slice() != core::slice::from_ref(&other.pool_id)
            || other.class_id.as_str()
                != conduit_i2c_transaction::installation::I2C_ATTACHMENT_CLASS
            || other.capacity_units != 1
            || other.compute.is_some()
            || other.content.is_some()
        {
            return Err(Refusal::Context);
        }
    }
    let native = if publish_native {
        let (ready, identity_bytes) =
            i2c_installation::prepare(actual, maximum_bytes).map_err(Refusal::Native)?;
        let projection = ready
            .project_for_images(
                actual,
                maximum_bytes
                    .checked_sub(identity_bytes)
                    .ok_or(Refusal::Capacity)?,
            )
            .map_err(|_| Refusal::Projection)?;
        // End the native loan before returning a publication or awaiting carrier
        // input. Diagnostics remain independent; Start must reacquire possession.
        Some(projection)
    } else {
        None
    };
    let native_bytes = native.as_ref().map_or(0, |n| n.owned_bytes);
    let mut remaining = maximum_bytes
        .checked_sub(native_bytes)
        .ok_or(Refusal::Capacity)?;
    let resources = 1 + usize::from(native.is_some());
    let bases = usize::from(native.is_some());
    let required = [
        actual.host_id.as_str(),
        actual.boot_id.as_str(),
        actual.profile.as_str(),
        resident.pool_id.as_str(),
        resident.class_id.as_str(),
    ]
    .iter()
    .try_fold(0usize, |n, s| n.checked_add(s.len()))
    .and_then(|n| n.checked_add(resources.checked_mul(size_of::<c::ResourceOffer>())?))
    .and_then(|n| n.checked_add(bases.checked_mul(size_of::<c::BaseProviderAdvertisement>())?))
    .ok_or(Refusal::Capacity)?;
    if required > remaining {
        return Err(Refusal::Capacity);
    }
    let host_id = copy(actual.host_id.as_str(), &mut remaining)?.into();
    let boot_id = copy(actual.boot_id.as_str(), &mut remaining)?.into();
    let profile = copy(actual.profile.as_str(), &mut remaining)?.into();
    let pool_id = copy(resident.pool_id.as_str(), &mut remaining)?.into();
    let class_id = copy(resident.class_id.as_str(), &mut remaining)?.into();
    let mut resource_offers = vector(resources, &mut remaining)?;
    let mut base_offers = vector(bases, &mut remaining)?;
    resource_offers.push(c::ResourceOffer {
        pool_id,
        class_id,
        capacity_units: resident.capacity_units,
        compute: None,
        content: None,
    });
    if let Some(native) = native {
        base_offers.push(native.base);
        resource_offers.push(native.resource);
    }
    Ok(c::HostAdvertisement {
        declared_name: None,
        protocol_version: actual.protocol_version,
        host_id,
        boot_id,
        offer_generation: c::OfferGeneration(generation),
        profile,
        bases: base_offers,
        resources: resource_offers,
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    })
}
fn copy(text: &str, remaining: &mut usize) -> Result<String, Refusal> {
    if text.len() > *remaining {
        return Err(Refusal::Capacity);
    }
    let mut value = String::new();
    value
        .try_reserve_exact(text.len())
        .map_err(|_| Refusal::Allocation)?;
    *remaining = remaining
        .checked_sub(value.capacity())
        .ok_or(Refusal::Capacity)?;
    value.push_str(text);
    Ok(value)
}
fn vector<T>(count: usize, remaining: &mut usize) -> Result<Vec<T>, Refusal> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Refusal::Allocation)?;
    *remaining = remaining
        .checked_sub(
            values
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or(Refusal::Capacity)?,
        )
        .ok_or(Refusal::Capacity)?;
    Ok(values)
}
