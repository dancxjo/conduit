//! Borrow only original initialized native custody; never decoded metadata.
use super::*;
impl<'a> NativeFacts<'a> {
    pub fn renderer<C: crate::execution_mask::RendererClock>(
        owner: &'a crate::execution_mask::ReadyRenderer<C>,
    ) -> Self {
        facts!(owner.native_identity(), crate::execution_mask::CAPABILITY)
    }
    pub fn pulse<P: conduit_pulse_capture::owner::PulseReadProvider>(
        owner: &'a conduit_pulse_capture::installation::ReadyPulseBase<P>,
    ) -> Self {
        facts!(
            owner.identity(),
            conduit_pulse_capture::installation::PULSE_CAPABILITY
        )
    }
    pub fn write<P: conduit_tm1637_wire::OpenDrain>(
        owner: &'a conduit_tm1637_wire::installation::ReadyWriteBase<P>,
    ) -> Self {
        facts!(
            owner.native_identity(),
            conduit_tm1637_wire::installation::WRITE_CAPABILITY
        )
    }
    pub fn wait<P: conduit_monotonic_wait::MonotonicWaitProvider>(
        owner: &'a conduit_monotonic_wait::ReadyMonotonicWaitBase<P>,
    ) -> Self {
        facts!(
            owner.native_identity(),
            conduit_monotonic_wait::WAIT_CAPABILITY
        )
    }
    /// Borrow actual exclusive native custody. Linked identifiers describe this
    /// installed implementation; no offer or incoming assignment supplies facts.
    pub fn i2c<P: conduit_i2c_transaction::I2cProvider>(
        owner: &'a conduit_i2c_transaction::installation::ReadyI2cBase<P>,
    ) -> Self {
        let id = owner.native_identity();
        NativeFacts {
            host: &id.host_id,
            boot: &id.boot_id,
            instance: &id.provider_instance_id,
            generation: id.provider_generation,
            pool: &id.resource_pool_id,
            resource_generation: &id.resource_generation_id,
            envelope: &id.envelope_id,
            artifact: &id.artifact_id,
            implementation: conduit_i2c_transaction::installation::I2C_IMPLEMENTATION,
            capability: conduit_i2c_transaction::installation::I2C_CAPABILITY,
        }
    }
    pub fn clock_at<P: conduit_monotonic_wait::clock_at::owner::MonotonicDeadlineProvider>(
        owner: &'a conduit_monotonic_wait::clock_at::retained::ReadyClockAtBase<P>,
    ) -> Self {
        let id = owner.identity();
        facts!(id, id.capability_id.as_str())
    }
}
