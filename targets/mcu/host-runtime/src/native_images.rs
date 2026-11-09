//! Compact actual Ready-provider facts beside separately fabricated flash images.
//! No decoded capability schemas, execution handles, reservations or grants.
extern crate alloc;
use alloc::{string::String, vec::Vec};
use conduit_core as c;
use core::mem::size_of;
pub struct Projection {
    pub base: c::BaseProviderAdvertisement,
    pub resource: c::ResourceOffer,
    pub owned_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Context,
    Capacity,
    Allocation,
}
/// Funds the containing vectors and every compact Ready projection once.
/// A failed construction leaves metadata incomplete: its caller must discard
/// that advertisement, never install/publish it as a successful Host offer.
pub struct Publication<'a> {
    actual: &'a mut c::HostAdvertisement,
    remaining: usize,
    expected: usize,
    appended: usize,
}
impl<'a> Publication<'a> {
    pub fn new(
        actual: &'a mut c::HostAdvertisement,
        count: usize,
        maximum_bytes: usize,
    ) -> Result<Self, Refusal> {
        let outer = count
            .checked_mul(size_of::<c::BaseProviderAdvertisement>())
            .and_then(|n| {
                count
                    .checked_mul(size_of::<c::ResourceOffer>())
                    .and_then(|r| n.checked_add(r))
            })
            .ok_or(Refusal::Capacity)?;
        if outer > maximum_bytes {
            return Err(Refusal::Capacity);
        }
        let old_bases = actual.bases.capacity();
        let old_resources = actual.resources.capacity();
        actual
            .bases
            .try_reserve_exact(count)
            .map_err(|_| Refusal::Allocation)?;
        actual
            .resources
            .try_reserve_exact(count)
            .map_err(|_| Refusal::Allocation)?;
        let actual_outer = (actual.bases.capacity() - old_bases)
            .checked_mul(size_of::<c::BaseProviderAdvertisement>())
            .and_then(|n| {
                (actual.resources.capacity() - old_resources)
                    .checked_mul(size_of::<c::ResourceOffer>())
                    .and_then(|r| n.checked_add(r))
            })
            .ok_or(Refusal::Capacity)?;
        let remaining = maximum_bytes
            .checked_sub(actual_outer)
            .ok_or(Refusal::Capacity)?;
        Ok(Self {
            actual,
            remaining,
            expected: count,
            appended: 0,
        })
    }
    fn append(&mut self, projection: Projection) -> Result<(), Refusal> {
        if self.appended >= self.expected {
            return Err(Refusal::Capacity);
        }
        self.remaining = self
            .remaining
            .checked_sub(projection.owned_bytes)
            .ok_or(Refusal::Capacity)?;
        self.actual.bases.push(projection.base);
        self.actual.resources.push(projection.resource);
        self.appended += 1;
        Ok(())
    }
    pub fn pulse<P: conduit_pulse_capture::owner::PulseReadProvider>(
        &mut self,
        owner: &conduit_pulse_capture::installation::ReadyPulseBase<P>,
    ) -> Result<(), Refusal> {
        self.append(pulse(owner, self.actual, self.remaining)?)
    }
    pub fn write<P: conduit_tm1637_wire::OpenDrain>(
        &mut self,
        owner: &conduit_tm1637_wire::installation::ReadyWriteBase<P>,
    ) -> Result<(), Refusal> {
        self.append(write(owner, self.actual, self.remaining)?)
    }
    pub fn wait<P: conduit_monotonic_wait::MonotonicWaitProvider>(
        &mut self,
        owner: &conduit_monotonic_wait::ReadyMonotonicWaitBase<P>,
    ) -> Result<(), Refusal> {
        self.append(wait(owner, self.actual, self.remaining)?)
    }
    pub fn renderer<P: crate::execution_mask::RendererClock>(
        &mut self,
        owner: &crate::execution_mask::ReadyRenderer<P>,
    ) -> Result<(), Refusal> {
        self.append(renderer(owner, self.actual, self.remaining)?)
    }
    pub fn finish(self) -> Result<(), Refusal> {
        if self.appended != self.expected {
            return Err(Refusal::Context);
        }
        // Canonical public resource order is an ordinary planner contract. Do
        // this before Root owns/signs the initial advertisement; indices in
        // subsequently prepared Plans then name that same actual inventory.
        self.actual
            .resources
            .sort_unstable_by(|a, b| a.pool_id.cmp(&b.pool_id));
        self.actual
            .bases
            .sort_unstable_by(|a, b| a.base_id.cmp(&b.base_id));
        Ok(())
    }
}
struct Facts<'a> {
    host: &'a c::HostId,
    boot: &'a c::BootId,
    base: &'a c::HostBaseId,
    instance: &'a c::BaseInstanceId,
    generation: u64,
    implementation: &'a c::BaseImplementationId,
    pool: &'a c::ResourcePoolId,
    class: &'static str,
    mechanism: &'static str,
    capability: &'static str,
}
macro_rules! facts {
    ($id:expr,$class:expr,$capability:expr) => {{
        let id = $id;
        Facts {
            host: &id.host_id,
            boot: &id.boot_id,
            base: &id.base_id,
            instance: &id.provider_instance_id,
            generation: id.provider_generation,
            implementation: &id.base_implementation_id,
            pool: &id.resource_pool_id,
            class: $class,
            mechanism: $class,
            capability: $capability,
        }
    }};
}
pub fn pulse<P: conduit_pulse_capture::owner::PulseReadProvider>(
    owner: &conduit_pulse_capture::installation::ReadyPulseBase<P>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    project(
        facts!(
            owner.identity(),
            conduit_pulse_capture::installation::PULSE_ATTACHMENT_CLASS,
            conduit_pulse_capture::installation::PULSE_CAPABILITY
        ),
        actual,
        maximum_bytes,
    )
}
pub fn write<P: conduit_tm1637_wire::OpenDrain>(
    owner: &conduit_tm1637_wire::installation::ReadyWriteBase<P>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    project(
        facts!(
            owner.native_identity(),
            conduit_tm1637_wire::installation::WRITE_ATTACHMENT_CLASS,
            conduit_tm1637_wire::installation::WRITE_CAPABILITY
        ),
        actual,
        maximum_bytes,
    )
}
pub fn wait<P: conduit_monotonic_wait::MonotonicWaitProvider>(
    owner: &conduit_monotonic_wait::ReadyMonotonicWaitBase<P>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    project(
        facts!(
            owner.native_identity(),
            conduit_monotonic_wait::WAIT_ATTACHMENT_CLASS,
            conduit_monotonic_wait::WAIT_CAPABILITY
        ),
        actual,
        maximum_bytes,
    )
}
pub fn renderer<P: crate::execution_mask::RendererClock>(
    owner: &crate::execution_mask::ReadyRenderer<P>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    project(
        facts!(
            owner.native_identity(),
            crate::execution_mask::ATTACHMENT,
            crate::execution_mask::CAPABILITY
        ),
        actual,
        maximum_bytes,
    )
}
fn project(
    f: Facts<'_>,
    actual: &c::HostAdvertisement,
    maximum: usize,
) -> Result<Projection, Refusal> {
    if actual.host_id != *f.host
        || actual.boot_id != *f.boot
        || f.generation == 0
        || actual
            .bases
            .iter()
            .any(|b| b.base_id == *f.base || b.provider_instance_id == *f.instance)
        || actual.resources.iter().any(|r| r.pool_id == *f.pool)
    {
        return Err(Refusal::Context);
    }
    let fields = [
        f.base.as_str(),
        f.instance.as_str(),
        f.implementation.as_str(),
        f.mechanism,
        f.capability,
        f.pool.as_str(),
        f.pool.as_str(),
        f.class,
    ];
    let vectors = size_of::<c::CapabilityId>()
        .checked_add(size_of::<c::ResourcePoolId>())
        .ok_or(Refusal::Capacity)?;
    let required = fields
        .iter()
        .try_fold(vectors, |n, s| {
            if s.is_empty() {
                None
            } else {
                n.checked_add(s.len())
            }
        })
        .ok_or(Refusal::Capacity)?;
    if required > maximum {
        return Err(Refusal::Capacity);
    }
    let mut remaining = maximum;
    let base_id = copy(fields[0], &mut remaining)?.into();
    let provider_instance_id = copy(fields[1], &mut remaining)?.into();
    let implementation_id = copy(fields[2], &mut remaining)?.into();
    let mechanism_family = copy(fields[3], &mut remaining)?.into();
    let capability = copy(fields[4], &mut remaining)?.into();
    let resource = copy(fields[5], &mut remaining)?.into();
    let pool_id = copy(fields[6], &mut remaining)?.into();
    let class_id = copy(fields[7], &mut remaining)?.into();
    let mut capability_ids = vector(1, &mut remaining)?;
    capability_ids.push(capability);
    let mut resource_pool_ids = vector(1, &mut remaining)?;
    resource_pool_ids.push(resource);
    Ok(Projection {
        base: c::BaseProviderAdvertisement {
            base_id,
            provider_instance_id,
            provider_generation: f.generation,
            implementation_id,
            mechanism_family,
            enforcement_class: c::BaseEnforcementClass::Cooperative,
            lifecycle: c::BaseLifecycle::Ready,
            capability_ids,
            resource_pool_ids,
        },
        resource: c::ResourceOffer {
            pool_id,
            class_id,
            capacity_units: 1,
            compute: None,
            content: None,
        },
        owned_bytes: maximum - remaining,
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
    if count.checked_mul(size_of::<T>()).ok_or(Refusal::Capacity)? > *remaining {
        return Err(Refusal::Capacity);
    }
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

#[path = "native_images_append.rs"]
mod append;
pub use append::{record_append, recovered_record_append};

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_pulse_capture::{
        installation::{PulseNativeIdentity, ReadyPulseBase},
        owner::{PulseAttachment, PulseReadProvider},
        *,
    };
    struct BoundedFixture;
    impl PulseReadProvider for BoundedFixture {
        async fn read(
            &mut self,
            _: CheckedPulseRead,
        ) -> Result<PulseReadCapture, PulseReadRefusal> {
            Err(PulseReadRefusal::DriveExtent)
        }
    }
    fn ad() -> c::HostAdvertisement {
        c::HostAdvertisement {
            declared_name: None,
            protocol_version: c::PROTOCOL_VERSION,
            host_id: "test/host".into(),
            boot_id: "test/boot".into(),
            offer_generation: c::OfferGeneration(1),
            profile: "test/profile".into(),
            bases: Vec::new(),
            resources: Vec::new(),
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        }
    }
    fn ready() -> ReadyPulseBase<BoundedFixture> {
        // SAFETY: owned bounded fixture cannot touch hardware or retain IO.
        unsafe {
            ReadyPulseBase::new(
                PulseNativeIdentity {
                    host_id: "test/host".into(),
                    boot_id: "test/boot".into(),
                    base_id: "test/base".into(),
                    provider_instance_id: "test/instance".into(),
                    provider_generation: 1,
                    resource_pool_id: "test/pool".into(),
                    resource_generation_id: c::ResourceGenerationId(
                        "test/resource-generation".into(),
                    ),
                    envelope_id: "test/envelope".into(),
                    artifact_id: "test/artifact".into(),
                    implementation_id: "test/implementation".into(),
                    base_implementation_id: "test/base-implementation".into(),
                },
                PulseAttachment {
                    generation: 1,
                    resource_bytes: 8,
                    limits: PulseReadLimits {
                        maximum_hold_low_us: 50000,
                        maximum_skipped_pulses: 4,
                        minimum_sample_offset_us: 10,
                        maximum_sample_offset_us: 200,
                        maximum_octets: 8,
                        maximum_capture_timeout_us: 50000,
                    },
                },
                BoundedFixture,
            )
            .unwrap()
        }
    }
    #[test]
    fn publication_funds_outer_vectors_and_requires_complete_ready_set() {
        let owner = ready();
        let mut actual = ad();
        assert!(matches!(
            Publication::new(&mut actual, 1, 0),
            Err(Refusal::Capacity)
        ));
        assert_eq!(actual.bases.capacity(), 0);
        let publication = Publication::new(&mut actual, 1, 4096).unwrap();
        assert_eq!(publication.finish(), Err(Refusal::Context));
        let mut publication = Publication::new(&mut actual, 1, 4096).unwrap();
        publication.pulse(&owner).unwrap();
        publication.finish().unwrap();
        assert_eq!(actual.bases.len(), 1);
        assert_eq!(actual.resources.len(), 1);
        assert!(
            actual.capabilities.is_empty(),
            "no decoded capability retention"
        );
    }
    #[test]
    fn compact_projection_preserves_exact_ready_instance_and_refuses_scope_capacity_duplicates() {
        let owner = ready();
        let mut actual = ad();
        assert!(matches!(pulse(&owner, &actual, 0), Err(Refusal::Capacity)));
        let projection = pulse(&owner, &actual, 4096).unwrap();
        assert!(projection.owned_bytes > 0 && projection.owned_bytes <= 4096);
        assert_eq!(
            projection.base.provider_instance_id,
            owner.identity().provider_instance_id
        );
        assert_eq!(
            projection.base.implementation_id,
            owner.identity().base_implementation_id
        );
        assert_eq!(projection.base.lifecycle, c::BaseLifecycle::Ready);
        assert_eq!(
            projection.resource.pool_id,
            owner.identity().resource_pool_id
        );
        assert_eq!(projection.resource.capacity_units, 1);
        assert!(actual.capabilities.is_empty());
        actual.boot_id = "test/another-boot".into();
        assert!(matches!(
            pulse(&owner, &actual, 4096),
            Err(Refusal::Context)
        ));
        actual = ad();
        actual.resources.push(projection.resource);
        assert!(matches!(
            pulse(&owner, &actual, 4096),
            Err(Refusal::Context)
        ));
        actual = ad();
        actual.bases.push(projection.base);
        assert!(matches!(
            pulse(&owner, &actual, 4096),
            Err(Refusal::Context)
        ));
    }
}
