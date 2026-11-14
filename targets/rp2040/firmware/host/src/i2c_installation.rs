//! Ready native ownership from checked local construction and current Root facts.
//! No capability handle, resource reservation, Line or Play is issued here.
use crate::{
    config,
    i2c_owner::{self, NativeBusLease},
};
use alloc::string::String;
use conduit_core::{HostAdvertisement, PROTOCOL_VERSION};
use conduit_i2c_transaction::{
    installation::{I2cNativeIdentity, ReadyI2cBase},
    owner::I2cAttachment,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Unselected,
    Context,
    Unavailable,
    Identity,
    Capacity,
    Allocation,
}

/// Root supplies its actual Host and remaining preparation budget. The returned
/// charge is retained identity backing only; codecs, offers, grants and kernels
/// still need their independently admitted storage. Nothing performs bus IO.
pub fn prepare(
    actual: &HostAdvertisement,
    maximum_bytes: usize,
) -> Result<(ReadyI2cBase<NativeBusLease>, usize), Refusal> {
    if config::I2C_CALL_MAXIMUM_OPERATIONS == 0 {
        return Err(Refusal::Unselected);
    }
    let (minimum_address, maximum_address) =
        config::I2C_CALL_ADDRESS_RANGE.ok_or(Refusal::Unselected)?;
    if actual.protocol_version != PROTOCOL_VERSION
        || actual.host_id.as_str().is_empty()
        || actual.boot_id.as_str().is_empty()
        || actual.offer_generation.0 == 0
        || actual.profile.as_str() != config::CONFIGURATION_ID
        || !(8..=119).contains(&minimum_address)
        || minimum_address > maximum_address
        || maximum_address > 119
    {
        return Err(Refusal::Context);
    }
    let (base, suffix, pool, generation_suffix, envelope_suffix) = match config::I2C_CONTROLLER {
        0 => (
            "rp2040/i2c/0",
            "/i2c/0",
            "pool/rp2040-i2c-0",
            "/i2c/0/1",
            "/i2c/0/calls",
        ),
        1 => (
            "rp2040/i2c/1",
            "/i2c/1",
            "pool/rp2040-i2c-1",
            "/i2c/1/1",
            "/i2c/1/calls",
        ),
        _ => return Err(Refusal::Context),
    };
    let base_implementation = config::I2C_BASE_IMPLEMENTATION.ok_or(Refusal::Unselected)?;
    let boot = actual.boot_id.as_str();
    let fields: [&[&str]; 9] = [
        &[actual.host_id.as_str()],
        &[boot],
        &[base],
        &[boot, suffix],
        &[pool],
        &[boot, generation_suffix],
        &[boot, envelope_suffix],
        &[config::IMAGE_ID],
        &[base_implementation],
    ];
    let required = fields
        .iter()
        .flat_map(|parts| parts.iter())
        .try_fold(0usize, |total, text| total.checked_add(text.len()))
        .ok_or(Refusal::Capacity)?;
    if required > maximum_bytes {
        return Err(Refusal::Capacity);
    }
    let provider = i2c_owner::try_native_lease().ok_or(Refusal::Unavailable)?;
    let mut owned_bytes = 0usize;
    let mut copy = |parts: &[&str]| -> Result<String, Refusal> {
        let length = parts
            .iter()
            .try_fold(0usize, |n, text| n.checked_add(text.len()))
            .ok_or(Refusal::Capacity)?;
        let mut text = String::new();
        text.try_reserve_exact(length)
            .map_err(|_| Refusal::Allocation)?;
        if owned_bytes
            .checked_add(text.capacity())
            .is_none_or(|n| n > maximum_bytes)
        {
            return Err(Refusal::Capacity);
        }
        owned_bytes += text.capacity();
        for part in parts {
            text.push_str(part);
        }
        Ok(text)
    };
    let identity = I2cNativeIdentity {
        base_implementation_id: copy(fields[8])?.into(),
        host_id: copy(fields[0])?.into(),
        boot_id: copy(fields[1])?.into(),
        base_id: copy(fields[2])?.into(),
        provider_instance_id: copy(fields[3])?.into(),
        provider_generation: 1,
        resource_pool_id: copy(fields[4])?.into(),
        resource_generation_id: conduit_core::ResourceGenerationId(copy(fields[5])?),
        envelope_id: copy(fields[6])?.into(),
        artifact_id: copy(fields[7])?.into(),
    };
    // SAFETY: the lease holds this Boot's actually configured controller from
    // the sole native constructor. The checked local address interval is not
    // supplied by an incoming Plot. Rp2040I2c bounds polls and quarantines faults.
    // Root still independently issues exact local handles and admits the workset.
    let ready = unsafe {
        ReadyI2cBase::new(
            identity,
            I2cAttachment {
                generation: 1,
                minimum_address,
                maximum_address,
                resource_bytes: conduit_i2c_transaction::transaction::MAXIMUM_TRANSACTION_BYTES
                    as u64,
            },
            provider,
        )
    }
    .map_err(|_| Refusal::Identity)?;
    Ok((ready, owned_bytes))
}
