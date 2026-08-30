use conduit_create_oi::{CreateUartProvider, UartProfile};
use embedded_hal_v0::serial::{Read, Write};

/// ATmega32U4 mechanism adapter for the shared Create OI implementation.
///
/// It owns no Create opcode, Plan identity, lifecycle, or retry policy.
pub struct AvrCreateUart<U> {
    uart: U,
}

impl<U> AvrCreateUart<U> {
    pub const fn new(uart: U) -> Self {
        Self { uart }
    }
}

impl<U, E> CreateUartProvider for AvrCreateUart<U>
where
    U: Read<u8, Error = E> + Write<u8, Error = E>,
{
    type Error = E;

    fn is_available(&self) -> bool {
        true
    }

    fn profile(&self) -> UartProfile {
        UartProfile::CREATE_OI
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        for byte in bytes {
            nb::block!(self.uart.write(*byte))?;
        }
        nb::block!(self.uart.flush())
    }

    fn read_byte(&mut self, _deadline_tick: u64) -> Result<Option<u8>, Self::Error> {
        match self.uart.read() {
            Ok(byte) => Ok(Some(byte)),
            Err(nb::Error::WouldBlock) => Ok(None),
            Err(nb::Error::Other(error)) => Err(error),
        }
    }
}
