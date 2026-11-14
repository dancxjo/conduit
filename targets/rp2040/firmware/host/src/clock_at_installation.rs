//! One original boot-relative millisecond reader, separate from timer grants.
//! Construction/publishing never performs a read or authorizes a Host Call.
use alloc::string::String;
use conduit_core::{HostAdvertisement, ResourceGenerationId, PROTOCOL_VERSION};
use conduit_monotonic_wait::clock_at::{
    codec::ClockDisposition,
    installation::{
        CLOCK_AUTHORITY, CLOCK_EXECUTION_PROFILE, CLOCK_IMPLEMENTATION, CLOCK_RESOURCE_CLASS,
    },
    owner::MonotonicDeadlineProvider,
    polling::PollingClock,
    retained::{ClockAtAttachment, ClockAtNativeIdentity, ReadyClockAtBase},
};
use core::cell::Cell;
static SLOT: critical_section::Mutex<Cell<bool>> = critical_section::Mutex::new(Cell::new(false));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Unselected,
    Context,
    Busy,
    Capacity,
    Allocation,
    Identity,
}
pub struct ClockLease {
    reader: PollingClock,
}
impl ClockLease {
    fn acquire() -> Result<Self, Refusal> {
        critical_section::with(|cs| {
            if SLOT.borrow(cs).replace(true) {
                return Err(Refusal::Busy);
            }
            Ok(Self {
                reader: PollingClock::new(read_boot_millis),
            })
        })
    }
}
fn read_boot_millis() -> Result<u64, ClockDisposition> {
    Ok(embassy_time::Instant::now().as_millis())
}
impl MonotonicDeadlineProvider for ClockLease {
    fn poll_until(&mut self, deadline_ms: u64) -> Result<Option<u64>, ClockDisposition> {
        self.reader.poll_until(deadline_ms)
    }
    fn sample_now(&mut self) -> Result<u64, ClockDisposition> {
        self.reader.sample_now()
    }
    fn revoke(&mut self) {
        self.reader.revoke();
    }
}
impl Drop for ClockLease {
    fn drop(&mut self) {
        critical_section::with(|cs| SLOT.borrow(cs).set(false));
    }
}
/// Root transfers current Host/Boot and checked IMAGE construction only. Incoming
/// Plans supply none of these identity fields, calibration or logical slot.
pub fn prepare(
    actual: &HostAdvertisement,
    maximum_bytes: usize,
) -> Result<(ReadyClockAtBase<ClockLease>, usize), Refusal> {
    let base_implementation =
        crate::config::CLOCK_AT_BASE_IMPLEMENTATION.ok_or(Refusal::Unselected)?;
    if crate::config::CLOCK_AT_MAXIMUM_CALL_OPERATIONS == 0 || crate::config::CLOCK_AT_SLOTS == 0 {
        return Err(Refusal::Unselected);
    }
    if actual.protocol_version != PROTOCOL_VERSION
        || actual.host_id.as_str().is_empty()
        || actual.boot_id.as_str().is_empty()
        || actual.offer_generation.0 == 0
        || actual.profile.as_str() != crate::config::CONFIGURATION_ID
    {
        return Err(Refusal::Context);
    }
    let mut backing = 0usize;
    let mut copy = |parts: &[&str]| -> Result<String, Refusal> {
        let length = parts
            .iter()
            .try_fold(0usize, |n, part| n.checked_add(part.len()))
            .ok_or(Refusal::Capacity)?;
        if length > maximum_bytes.saturating_sub(backing) {
            return Err(Refusal::Capacity);
        }
        let mut text = String::new();
        text.try_reserve_exact(length)
            .map_err(|_| Refusal::Allocation)?;
        backing = backing
            .checked_add(text.capacity())
            .filter(|n| *n <= maximum_bytes)
            .ok_or(Refusal::Capacity)?;
        for part in parts {
            text.push_str(part);
        }
        Ok(text)
    };
    let identity = ClockAtNativeIdentity {
        host_id: copy(&[actual.host_id.as_str()])?.into(),
        boot_id: copy(&[actual.boot_id.as_str()])?.into(),
        base_id: copy(&["rp2040/clock-at/0"])?.into(),
        provider_instance_id: copy(&[actual.boot_id.as_str(), "/clock-at/0"])?.into(),
        provider_generation: 1,
        resource_pool_id: copy(&["pool/rp2040/clock-at/0"])?.into(),
        resource_generation_id: ResourceGenerationId(copy(&[
            actual.boot_id.as_str(),
            "/clock-at/0/1",
        ])?),
        envelope_id: copy(&[actual.boot_id.as_str(), "/clock-at/0/calls"])?.into(),
        artifact_id: copy(&[crate::config::IMAGE_ID])?.into(),
        implementation_id: copy(&[CLOCK_IMPLEMENTATION])?.into(),
        base_implementation_id: copy(&[base_implementation])?.into(),
        capability_id: copy(&[CLOCK_IMPLEMENTATION])?.into(),
        execution_profile_id: copy(&[CLOCK_EXECUTION_PROFILE])?.into(),
        authority_contract_id: copy(&[CLOCK_AUTHORITY])?.into(),
        mechanism_family: copy(&[CLOCK_RESOURCE_CLASS])?,
        resource_class: copy(&[CLOCK_RESOURCE_CLASS])?,
    };
    let provider = ClockLease::acquire()?;
    // SAFETY: the affine lease owns this Boot's one selected logical reader;
    // Embassy's configured RP2040 monotonic driver supplies calibrated boot-ms.
    // Identity backing is bounded above, no timer/observation/effect occurs here,
    // and original Root authorization/admission remains independently required.
    let ready = unsafe {
        ReadyClockAtBase::new(identity, ClockAtAttachment { resource_bytes: 1 }, provider)
    }
    .map_err(|_| Refusal::Identity)?;
    Ok((ready, backing))
}
