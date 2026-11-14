//! Current local private-work evidence after initialized inventory preflight.
use alloc::string::String;
use conduit_body::DelegatedResourceRefusal as E;
use conduit_core as c;
use core::fmt::Write;

pub struct WorkObservation {
    pub value: c::ResourceObservation,
    pub owned_bytes: usize,
}

/// The opaque preparation holds the actual initialized inventory exclusively.
/// Only the Source-selected private computation pool is observed, with no
/// existing reservations. This is not evidence of physical IO readiness.
pub fn observe_private_work(
    _prepared: &super::AttemptStorage<'_, '_>,
    owner: &c::ResourceAdmissionOwner,
    pool_id: &str,
    capacity: u32,
    sequence: u64,
    maximum_bytes: usize,
) -> Result<WorkObservation, E> {
    let actual = owner.host();
    let mut matching = actual
        .resources
        .iter()
        .filter(|pool| pool.pool_id.as_str() == pool_id);
    let pool = matching.next().ok_or(E::UnsupportedPool)?;
    if matching.next().is_some() {
        return Err(E::UnsupportedPool);
    }
    if !owner.admissions().is_empty() || sequence == 0 {
        return Err(E::Scope);
    }
    if pool.pool_id.as_str() != pool_id
        || pool.class_id.as_str() != c::COOPERATIVE_STEP_WORK_CLASS_ID
        || pool.capacity_units != capacity
        || capacity == 0
        || pool.compute.is_some()
        || pool.content.is_some()
    {
        return Err(E::UnsupportedPool);
    }
    let sign_capacity = actual
        .boot_id
        .as_str()
        .len()
        .checked_add(35)
        .ok_or(E::PreparationCapacity)?; // /resident-work/ plus twenty digits
    let texts = [
        actual.host_id.as_str(),
        actual.boot_id.as_str(),
        pool.pool_id.as_str(),
        pool.class_id.as_str(),
    ];
    let requested = texts
        .iter()
        .try_fold(sign_capacity, |n, text| n.checked_add(text.len()))
        .ok_or(E::PreparationCapacity)?;
    if requested > maximum_bytes {
        return Err(E::PreparationCapacity);
    }
    let mut owned_bytes = 0;
    let mut copy = |text: &str| -> Result<String, E> {
        let mut value = String::new();
        value
            .try_reserve_exact(text.len())
            .map_err(|_| E::Allocation)?;
        value.push_str(text);
        owned_bytes += value.capacity();
        Ok(value)
    };
    let host_id = copy(texts[0])?.into();
    let boot_id = copy(texts[1])?.into();
    let pool_id = copy(texts[2])?.into();
    let class_id = copy(texts[3])?.into();
    let mut sign = String::new();
    sign.try_reserve_exact(sign_capacity)
        .map_err(|_| E::Allocation)?;
    write!(
        &mut sign,
        "{}/resident-work/{sequence}",
        actual.boot_id.as_str()
    )
    .map_err(|_| E::PreparationCapacity)?;
    owned_bytes += sign.capacity();
    if owned_bytes > maximum_bytes {
        return Err(E::PreparationCapacity);
    }
    Ok(WorkObservation {
        owned_bytes,
        value: c::ResourceObservation {
            host_id,
            boot_id,
            pool_id,
            class_id,
            offer_generation: actual.offer_generation,
            health: c::ResourceHealth::Ready,
            unreserved_units: capacity,
            utilized_units: 0,
            sign_id: sign.into(),
        },
    })
}
