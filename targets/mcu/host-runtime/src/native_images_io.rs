//! Compact projections of the same retained native IO owners later bound.
use super::*;
use conduit_i2c_transaction::{
    installation::{I2cImageProjectionRefusal, ReadyI2cBase},
    I2cProvider,
};
use conduit_monotonic_wait::clock_at::{
    owner::MonotonicDeadlineProvider, retained::ReadyClockAtBase,
};
impl Publication<'_> {
    pub fn i2c<P: I2cProvider>(&mut self, owner: &ReadyI2cBase<P>) -> Result<(), Refusal> {
        self.append(i2c(owner, self.actual, self.remaining)?)
    }
    pub fn clock_at<P: MonotonicDeadlineProvider>(
        &mut self,
        owner: &ReadyClockAtBase<P>,
    ) -> Result<(), Refusal> {
        self.append(clock_at(owner, self.actual, self.remaining)?)
    }
}
pub fn i2c<P: I2cProvider>(
    owner: &ReadyI2cBase<P>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    let projection =
        owner
            .project_for_images(actual, maximum_bytes)
            .map_err(|reason| match reason {
                I2cImageProjectionRefusal::Context => Refusal::Context,
                I2cImageProjectionRefusal::Capacity => Refusal::Capacity,
                I2cImageProjectionRefusal::Allocation => Refusal::Allocation,
            })?;
    Ok(Projection {
        base: projection.base,
        resource: projection.resource,
        owned_bytes: projection.owned_bytes,
    })
}
pub fn clock_at<P: MonotonicDeadlineProvider>(
    owner: &ReadyClockAtBase<P>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    let id = owner.identity();
    project(
        Facts {
            host: &id.host_id,
            boot: &id.boot_id,
            base: &id.base_id,
            instance: &id.provider_instance_id,
            generation: id.provider_generation,
            implementation: &id.base_implementation_id,
            pool: &id.resource_pool_id,
            class: &id.resource_class,
            mechanism: &id.mechanism_family,
            capability: id.capability_id.as_str(),
        },
        actual,
        maximum_bytes,
    )
}
