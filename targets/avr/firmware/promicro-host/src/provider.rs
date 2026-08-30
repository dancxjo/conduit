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

    fn read_byte(&mut self, deadline_tick: u64) -> Result<Option<u8>, Self::Error> {
        // One provider tick is 10 microseconds on this exact adapter. The
        // shared transaction supplies the finite deadline; no retry policy or
        // baud probing is introduced here.
        for _ in 0..deadline_tick.min(2_000) {
            match self.uart.read() {
                Ok(byte) => return Ok(Some(byte)),
                Err(nb::Error::WouldBlock) => arduino_hal::delay_us(10),
                Err(nb::Error::Other(error)) => return Err(error),
            }
        }
        Ok(None)
    }
}
