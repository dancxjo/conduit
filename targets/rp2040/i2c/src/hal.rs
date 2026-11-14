//! Trusted native protocol drivers share the same finite bus owner.
//! This adapter supplies no membership, offer, assignment or authority.
use crate::Rp2040I2c;
use conduit_i2c_transaction::{I2cDisposition, I2cProvider, I2cTransaction};
use embassy_rp::i2c::Instance;
use embedded_hal::i2c::{ErrorKind, ErrorType, NoAcknowledgeSource};
use embedded_hal_async::i2c::{I2c, Operation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub I2cDisposition);
impl embedded_hal::i2c::Error for Error {
    fn kind(&self) -> ErrorKind {
        match self.0 {
            I2cDisposition::NotAcknowledged => {
                ErrorKind::NoAcknowledge(NoAcknowledgeSource::Unknown)
            }
            I2cDisposition::ArbitrationLost => ErrorKind::ArbitrationLoss,
            _ => ErrorKind::Other,
        }
    }
}
impl<T: Instance> ErrorType for Rp2040I2c<'_, T> {
    type Error = Error;
}
impl<T: Instance> I2c for Rp2040I2c<'_, T>
where
    Self: I2cProvider,
{
    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Error> {
        let (write, input): (&[u8], &mut [u8]) = match operations {
            [] => return Ok(()),
            [Operation::Read(input)] => (&[], input),
            [Operation::Write(write)] => (write, &mut []),
            [Operation::Write(write), Operation::Read(input)] => (write, input),
            // Refuse unsupported multi-phase geometry before any partial effect.
            _ => return Err(Error(I2cDisposition::Unsupported)),
        };
        let read = u8::try_from(input.len()).map_err(|_| Error(I2cDisposition::Refused))?;
        let request = I2cTransaction::new(address, write, read).map_err(Error)?;
        let actual = I2cProvider::transact(self, &request, input).map_err(Error)?;
        if actual != input.len() {
            return Err(Error(I2cDisposition::DeviceError));
        }
        Ok(())
    }
}
