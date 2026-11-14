//! Only an exclusively owned, already configured controller can enter this leaf.
use crate::polling;
use conduit_i2c_transaction::{I2cDisposition, I2cProvider, I2cTransaction};
use core::num::NonZeroU32;
use embassy_rp::{
    i2c::{AbortReason, Async, Error, I2c, Instance},
    pac, peripherals,
};

/// Consumes the actual HAL controller; no address discovery or device protocol.
/// The poll allowance bounds work, not wall time or electrical response latency.
/// A timed-out or failed controller is permanently quarantined until reconstructed
/// by its native owner. No later request implicitly resets or retries it.
pub struct Rp2040I2c<'d, T: Instance> {
    bus: I2c<'d, T, Async>,
    maximum_polls: NonZeroU32,
    revoked: bool,
}
impl<'d, T: Instance> Rp2040I2c<'d, T> {
    pub fn new(bus: I2c<'d, T, Async>, maximum_polls: NonZeroU32) -> Self {
        Self {
            bus,
            maximum_polls,
            revoked: false,
        }
    }
    /// Local quarantine state only; this does not prove a device is present or
    /// grant a caller authority to use the configured controller.
    pub fn is_revoked(&self) -> bool {
        self.revoked
    }
    fn execute(
        &mut self,
        request: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        if self.revoked {
            return Err(I2cDisposition::ProviderLost);
        }
        if input.len() != usize::from(request.read_length()) {
            return Err(I2cDisposition::Refused);
        }
        let bus = &mut self.bus;
        let transfer = async {
            match (request.write().is_empty(), input.is_empty()) {
                (true, false) => bus.read_async(request.address(), input).await,
                (false, true) => {
                    bus.write_async(request.address(), request.write().iter().copied())
                        .await
                }
                (false, false) => {
                    bus.write_read_async(request.address(), request.write().iter().copied(), input)
                        .await
                }
                (true, true) => unreachable!("checked nonempty transaction"),
            }
        };
        match polling::run(transfer, self.maximum_polls) {
            Some(Ok(())) => Ok(usize::from(request.read_length())),
            Some(Err(Error::Abort(AbortReason::NoAcknowledge))) => {
                Err(I2cDisposition::NotAcknowledged)
            }
            Some(Err(Error::Abort(AbortReason::ArbitrationLoss))) => {
                Err(I2cDisposition::ArbitrationLost)
            }
            Some(Err(_)) => Err(I2cDisposition::DeviceError),
            None => Err(I2cDisposition::TimedOut),
        }
    }
}
macro_rules! controller {
    ($instance:ident, $set_reset:ident, $get_reset:ident) => {
        impl Rp2040I2c<'_, peripherals::$instance> {
            fn quarantine(&mut self) -> bool {
                self.revoked = true;
                // No DMA is used by the HAL I2C implementation. Its future has
                // already been dropped: stop IRQ production, then hold only this
                // owned peripheral in reset. Never report an unobserved stop.
                pac::$instance
                    .ic_intr_mask()
                    .write_value(pac::i2c::regs::IcIntrMask(0));
                critical_section::with(|_| pac::RESETS.reset().modify(|r| r.$set_reset(true)));
                for _ in 0..self.maximum_polls.get() {
                    if pac::RESETS.reset().read().$get_reset()
                        && !pac::RESETS.reset_done().read().$get_reset()
                    {
                        return true;
                    }
                    core::hint::spin_loop();
                }
                false
            }
        }
        impl I2cProvider for Rp2040I2c<'_, peripherals::$instance> {
            fn transact(
                &mut self,
                request: &I2cTransaction<'_>,
                input: &mut [u8],
            ) -> Result<usize, I2cDisposition> {
                // Invalid storage is refused before polling, traffic or reset.
                if self.revoked {
                    return Err(I2cDisposition::ProviderLost);
                }
                if input.len() != usize::from(request.read_length()) {
                    return Err(I2cDisposition::Refused);
                }
                match self.execute(request, input) {
                    Ok(length) => Ok(length),
                    Err(reason) if self.quarantine() => Err(reason),
                    Err(_) => Err(I2cDisposition::ProviderLost),
                }
            }
            fn revoke(&mut self) {
                if !self.revoked {
                    self.quarantine();
                }
            }
        }
    };
}
controller!(I2C0, set_i2c0, i2c0);
controller!(I2C1, set_i2c1, i2c1);
