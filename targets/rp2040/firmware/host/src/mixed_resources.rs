//! Ready observations require the opaque preparation retaining both providers.
use super::*;
use alloc::string::String;
use b::DelegatedResourceRefusal as E;
use conduit_i2c_transaction::installation::I2C_ATTACHMENT_CLASS;
use core::fmt::Write;

pub struct OwnedObservations {
    values: [c::ResourceObservation; 2],
    pub owned_bytes: usize,
}
impl OwnedObservations {
    pub fn as_slice(&self) -> &[c::ResourceObservation] {
        &self.values
    }
}
impl<P: I2cProvider> PreparedMixedWorkset<'_, '_, '_, P> {
    /// No declaration by itself is Ready. This opaque preparation exclusively
    /// retains initialized resident inventory and checked native possession.
    /// No await, provider transfer or effect occurs between this witness and
    /// observation. Inline observation records remain a separate Root charge.
    pub fn observe_owned_resources(
        &self,
        owner: &c::ResourceAdmissionOwner,
        work_capacity: u32,
        sequence: u64,
        maximum_bytes: usize,
    ) -> Result<OwnedObservations, Refusal> {
        observe(self, owner, work_capacity, sequence, maximum_bytes).map_err(Refusal::Resources)
    }
}
impl<'w, 'n, 'o, P: I2cProvider> PreparedMixedWorkset<'w, 'n, 'o, P> {
    /// Fund observation backing and bridge/Core storage together before the
    /// existing atomic admission. Refusal drops only unstarted preparation;
    /// both providers and the original reservation ledger remain coherent.
    pub fn admit_observed(
        self,
        owner: &mut c::ResourceAdmissionOwner,
        work_capacity: u32,
        sequence: u64,
    ) -> Result<CommittedMixedWorkset<'w, 'o, P>, Refusal> {
        let available = self
            .maximum_storage_bytes
            .checked_sub(self.kernel.storage_bytes())
            .ok_or(Refusal::Bounds)?;
        let observations =
            self.observe_owned_resources(owner, work_capacity, sequence, available)?;
        let required = self
            .preparation_bytes(owner, observations.as_slice())?
            .checked_add(observations.owned_bytes)
            .ok_or(Refusal::Bounds)?;
        if required > self.maximum_storage_bytes {
            return Err(Refusal::Bounds);
        }
        self.admit_and_bind(owner, observations.as_slice())
    }
}
fn observe<P: I2cProvider>(
    prepared: &PreparedMixedWorkset<'_, '_, '_, P>,
    owner: &c::ResourceAdmissionOwner,
    work_capacity: u32,
    sequence: u64,
    maximum_bytes: usize,
) -> Result<OwnedObservations, E> {
    let actual = owner.host();
    let m = prepared.approved.metadata();
    if sequence == 0
        || work_capacity == 0
        || !owner.admissions().is_empty()
        || m.host != actual.host_id.as_str()
        || m.boot != actual.boot_id.as_str()
        || m.offer_generation != actual.offer_generation.0
        || prepared.approved.receiver_profile()
            != c::AssignedIdentity::from_text(actual.profile.as_str())
    {
        return Err(E::Scope);
    }
    let mut pools = [None; 2];
    let sign_bytes = actual
        .boot_id
        .as_str()
        .len()
        .checked_add("/mixed-resource/".len() + 20 + 2)
        .ok_or(E::PreparationCapacity)?;
    let mut required = 0usize;
    let mut count = 0;
    for entry in b::PlainFragmentResources::new(m.resource_bindings) {
        let entry = entry.map_err(|_| E::Projection)?;
        if count >= 2 {
            return Err(E::Scope);
        }
        let expected = match entry.class {
            c::COOPERATIVE_STEP_WORK_CLASS_ID => work_capacity,
            I2C_ATTACHMENT_CLASS => 1,
            _ => return Err(E::UnsupportedPool),
        };
        let mut matching = actual
            .resources
            .iter()
            .filter(|p| p.pool_id.as_str() == entry.pool);
        let pool = matching.next().ok_or(E::UnsupportedPool)?;
        if matching.next().is_some()
            || pool.class_id.as_str() != entry.class
            || pool.capacity_units != expected
            || entry.units > expected
            || pool.compute.is_some()
            || pool.content.is_some()
        {
            return Err(E::UnsupportedPool);
        }
        let text = [
            actual.host_id.as_str(),
            actual.boot_id.as_str(),
            pool.pool_id.as_str(),
            pool.class_id.as_str(),
        ];
        required = text
            .iter()
            .try_fold(required, |n, s| n.checked_add(s.len()))
            .and_then(|n| n.checked_add(sign_bytes))
            .ok_or(E::PreparationCapacity)?;
        pools[count] = Some(pool);
        count += 1;
    }
    if count != 2 {
        return Err(E::Scope);
    }
    if required > maximum_bytes {
        return Err(E::PreparationCapacity);
    }
    let mut remaining = maximum_bytes;
    let values = [
        observation(
            actual,
            pools[0].expect("checked first pool"),
            sequence,
            0,
            sign_bytes,
            &mut remaining,
        )?,
        observation(
            actual,
            pools[1].expect("checked second pool"),
            sequence,
            1,
            sign_bytes,
            &mut remaining,
        )?,
    ];
    Ok(OwnedObservations {
        values,
        owned_bytes: maximum_bytes - remaining,
    })
}
fn copy(text: &str, remaining: &mut usize) -> Result<String, E> {
    if text.len() > *remaining {
        return Err(E::PreparationCapacity);
    }
    let mut value = String::new();
    value
        .try_reserve_exact(text.len())
        .map_err(|_| E::Allocation)?;
    *remaining = remaining
        .checked_sub(value.capacity())
        .ok_or(E::PreparationCapacity)?;
    value.push_str(text);
    Ok(value)
}
fn observation(
    actual: &c::HostAdvertisement,
    pool: &c::ResourceOffer,
    sequence: u64,
    index: u8,
    sign_bytes: usize,
    remaining: &mut usize,
) -> Result<c::ResourceObservation, E> {
    let host_id = copy(actual.host_id.as_str(), remaining)?.into();
    let boot_id = copy(actual.boot_id.as_str(), remaining)?.into();
    let pool_id = copy(pool.pool_id.as_str(), remaining)?.into();
    let class_id = copy(pool.class_id.as_str(), remaining)?.into();
    if sign_bytes > *remaining {
        return Err(E::PreparationCapacity);
    }
    let mut sign = String::new();
    sign.try_reserve_exact(sign_bytes)
        .map_err(|_| E::Allocation)?;
    *remaining = remaining
        .checked_sub(sign.capacity())
        .ok_or(E::PreparationCapacity)?;
    write!(
        &mut sign,
        "{}/mixed-resource/{sequence}/{index}",
        actual.boot_id.as_str()
    )
    .map_err(|_| E::PreparationCapacity)?;
    Ok(c::ResourceObservation {
        host_id,
        boot_id,
        pool_id,
        class_id,
        offer_generation: actual.offer_generation,
        health: c::ResourceHealth::Ready,
        unreserved_units: pool.capacity_units,
        utilized_units: 0,
        sign_id: sign.into(),
    })
}
