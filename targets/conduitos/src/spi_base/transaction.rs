//! Exact finite exchange geometry, independent of peripheral command meanings.

pub const MAXIMUM_TRANSACTION_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpiDisposition {
    TimedOut,
    ProviderLost,
    Unsupported,
    StaleAttachment,
    Refused,
}

pub struct SpiTransaction<'a> {
    output: &'a [u8],
}

impl<'a> SpiTransaction<'a> {
    pub fn new(output: &'a [u8]) -> Result<Self, SpiDisposition> {
        if output.is_empty() || output.len() > MAXIMUM_TRANSACTION_BYTES {
            return Err(SpiDisposition::Refused);
        }
        Ok(Self { output })
    }

    pub fn output(&self) -> &[u8] {
        self.output
    }
}

/// The selected provider owns one immutable SPI attachment and configuration.
/// Requests cannot change chip select, mode, frequency or physical routing.
/// Success means all bytes shifted and exactly as many input bytes observed.
/// Every return must quiesce hardware or permanently quarantine the provider.
/// Revocation must be idempotent and enforce mandatory local safety.
pub trait SpiProvider {
    fn exchange(
        &mut self,
        request: &SpiTransaction<'_>,
        input: &mut [u8],
    ) -> Result<(), SpiDisposition>;
    fn revoke(&mut self);
}

impl<R: super::bcm2835::Registers> SpiProvider for super::bcm2835::Controller<R> {
    fn exchange(
        &mut self,
        request: &SpiTransaction<'_>,
        input: &mut [u8],
    ) -> Result<(), SpiDisposition> {
        super::bcm2835::Controller::exchange(self, request.output(), input).map_err(|reason| {
            use super::bcm2835::Refusal;
            match reason {
                Refusal::TimedOut => SpiDisposition::TimedOut,
                Refusal::ProviderLost => SpiDisposition::ProviderLost,
                Refusal::InvalidBounds | Refusal::InvalidGeometry | Refusal::Busy => {
                    SpiDisposition::Refused
                }
            }
        })
    }
    fn revoke(&mut self) {
        super::bcm2835::Controller::revoke(self);
    }
}
