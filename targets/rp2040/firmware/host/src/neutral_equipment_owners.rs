//! Generic retained native owners. Physical construction stays in the board adapter.
use conduit_core::HostAdvertisement;
use conduit_i2c_transaction::installation::ReadyI2cBase;
use conduit_monotonic_wait::clock_at::retained::ReadyClockAtBase;
pub struct Equipment<I, C> {
    pub bus: Option<ReadyI2cBase<I>>,
    pub clock: Option<ReadyClockAtBase<C>>,
    pub identity_bytes: usize,
}
impl<
        I: conduit_i2c_transaction::I2cProvider,
        C: conduit_monotonic_wait::clock_at::owner::MonotonicDeadlineProvider,
    > Equipment<I, C>
{
    pub fn publish(
        &self,
        actual: &mut HostAdvertisement,
        maximum_bytes: usize,
    ) -> Result<(), conduit_mcu_host_runtime::native_images::Refusal> {
        use conduit_mcu_host_runtime::native_images::{Publication, Refusal};
        let mut publication = Publication::new(actual, 2, maximum_bytes)?;
        publication.i2c(self.bus.as_ref().ok_or(Refusal::Context)?)?;
        publication.clock_at(self.clock.as_ref().ok_or(Refusal::Context)?)?;
        publication.finish()
    }
}
