//! Class-neutral finite I2C geometry and physical-provider boundary.

pub const MAXIMUM_TRANSACTION_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum I2cDisposition {
    DeviceError,
    NotAcknowledged,
    ArbitrationLost,
    TimedOut,
    ProviderLost,
    Unsupported,
    StaleAttachment,
    Refused,
}

pub struct I2cTransaction<'a> {
    address: u8,
    write: &'a [u8],
    read: u8,
}

impl<'a> I2cTransaction<'a> {
    pub fn new(address: u8, write: &'a [u8], read: u8) -> Result<Self, I2cDisposition> {
        if !(8..=119).contains(&address)
            || write.len() > MAXIMUM_TRANSACTION_BYTES
            || usize::from(read) > MAXIMUM_TRANSACTION_BYTES
            || (write.is_empty() && read == 0)
        {
            return Err(I2cDisposition::Refused);
        }
        Ok(Self {
            address,
            write,
            read,
        })
    }
    pub fn address(&self) -> u8 {
        self.address
    }
    pub fn write(&self) -> &[u8] {
        self.write
    }
    pub fn read_length(&self) -> u8 {
        self.read
    }
}

/// A selected low-level provider owns physics and bounded termination only.
///
/// The caller supplies exact finite storage. A combined write/read requires a
/// repeated START with no intermediate STOP. Unsupported geometry must refuse
/// before issuing traffic. Providers may neither inspect registers/opcodes as
/// device identities nor add protocol retries. Every return must leave hardware
/// quiescent, or quarantine the provider so no subsequent request or storage
/// reuse can occur. A timeout alone does not prove a controller has stopped.
pub trait I2cProvider {
    fn transact(
        &mut self,
        request: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition>;
    fn revoke(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_addresses_empty_requests_and_extent_are_refused() {
        for address in [0, 7, 120, 127, 128, 255] {
            assert!(matches!(
                I2cTransaction::new(address, &[1], 0),
                Err(I2cDisposition::Refused)
            ));
        }
        assert!(I2cTransaction::new(0x76, &[], 0).is_err());
        assert!(I2cTransaction::new(0x76, &[0; 33], 0).is_err());
        assert!(I2cTransaction::new(0x76, &[0], 33).is_err());
        for address in 8..=119 {
            let request = I2cTransaction::new(address, &[0; 32], 32).unwrap();
            assert_eq!(request.write().len(), 32);
            assert_eq!(request.read_length(), 32);
        }
    }
}
