//! Bounded polling SPI0 mechanism (BCM2835 ARM Peripherals, chapter 10).
//!
//! This layer neither maps registers nor grants possession. The machine owner
//! supplies an exclusive register window and configures pins before admission.
//! Poll limits bound work, not elapsed time. No peripheral is recognized here.

const CS: usize = 0;
const FIFO: usize = 4;
const CLK: usize = 8;
const CLEAR: u32 = 3 << 4;
const TA: u32 = 1 << 7;
const DONE: u32 = 1 << 16;
const RXD: u32 = 1 << 17;
const TXD: u32 = 1 << 18;
const MAX_BYTES: usize = 4096;

/// A machine-owned SPI0 register window, never an authored address.
///
/// # Safety
/// Implementors must provide exclusive, correctly ordered access to one SPI0
/// controller. No interrupt, DMA, firmware or other owner may access it during
/// this object's lifetime. Reads/writes must complete and preserve device order.
/// Pin routing, mapping/cacheability and electrical approval remain owner facts.
pub unsafe trait Registers {
    fn read(&mut self, offset: usize) -> u32;
    fn write(&mut self, offset: usize, value: u32);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    InvalidBounds,
    InvalidGeometry,
    Busy,
    TimedOut,
    ProviderLost,
}

/// Immutable admitted geometry. Divider 0 encodes 65536 per the hardware;
/// odd dividers refuse. Its relation to Hz needs an independently known clock.
#[derive(Clone, Copy)]
pub struct Configuration {
    pub chip_select: u8,
    pub clock_polarity: bool,
    pub clock_phase: bool,
    pub clock_divider: u16,
    pub maximum_bytes: usize,
    pub maximum_polls: u32,
}

pub struct Controller<R: Registers> {
    registers: R,
    configuration: Configuration,
    control: u32,
    revoked: bool,
}

impl<R: Registers> Controller<R> {
    pub fn admit(mut registers: R, configuration: Configuration) -> Result<Self, Refusal> {
        if configuration.maximum_bytes == 0
            || configuration.maximum_bytes > MAX_BYTES
            || configuration.maximum_polls == 0
        {
            return Err(Refusal::InvalidBounds);
        }
        if configuration.chip_select > 1 || configuration.clock_divider & 1 != 0 {
            return Err(Refusal::InvalidGeometry);
        }
        // Never clear another owner's transfer on admission.
        if registers.read(CS) & TA != 0 {
            return Err(Refusal::Busy);
        }
        let control = u32::from(configuration.chip_select)
            | (u32::from(configuration.clock_phase) << 2)
            | (u32::from(configuration.clock_polarity) << 3);
        registers.write(CS, control | CLEAR);
        registers.write(CLK, u32::from(configuration.clock_divider));
        Ok(Self {
            registers,
            configuration,
            control,
            revoked: false,
        })
    }

    /// Full-duplex exchange, exactly one received byte per transmitted byte.
    /// Write-only attachments still require RX draining; received bytes may be
    /// discarded by the caller. A partial operation is never returned as success.
    pub fn exchange(&mut self, output: &[u8], input: &mut [u8]) -> Result<(), Refusal> {
        if self.revoked {
            return Err(Refusal::ProviderLost);
        }
        if output.is_empty()
            || output.len() > self.configuration.maximum_bytes
            || output.len() != input.len()
        {
            return Err(Refusal::InvalidGeometry);
        }
        if self.registers.read(CS) & TA != 0 {
            self.revoked = true;
            return Err(Refusal::ProviderLost);
        }
        self.registers.write(CS, self.control | CLEAR);
        self.registers.write(CS, self.control | TA);
        let mut written = 0;
        let mut received = 0;
        for _ in 0..self.configuration.maximum_polls {
            let status = self.registers.read(CS);
            // Drain before filling so write-only users cannot wedge the RX FIFO.
            if status & RXD != 0 {
                if received >= written {
                    return self.fail(Refusal::ProviderLost);
                }
                input[received] = self.registers.read(FIFO) as u8;
                received += 1;
            }
            if written < output.len() && status & TXD != 0 {
                self.registers.write(FIFO, u32::from(output[written]));
                written += 1;
            }
            // DONE from before the final FIFO write cannot prove completion.
            if written == output.len()
                && received == output.len()
                && self.registers.read(CS) & DONE != 0
            {
                self.stop();
                return Ok(());
            }
            core::hint::spin_loop();
        }
        self.fail(Refusal::TimedOut)
    }

    /// Revoke permanently and deassert select even when no request is active.
    pub fn revoke(&mut self) {
        if !self.revoked {
            self.stop();
            self.revoked = true;
        }
    }

    fn stop(&mut self) {
        self.registers.write(CS, self.control | CLEAR);
    }

    fn fail(&mut self, reason: Refusal) -> Result<(), Refusal> {
        self.revoke();
        Err(reason)
    }
}

impl<R: Registers> Drop for Controller<R> {
    fn drop(&mut self) {
        if !self.revoked {
            self.stop();
        }
    }
}

#[cfg(test)]
#[path = "bcm2835_tests.rs"]
mod tests;
