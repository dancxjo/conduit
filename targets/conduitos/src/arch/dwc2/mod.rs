//! Class-neutral DWC2 controller preparation. No descriptors, HID policy or
//! device initialization requests live at this hardware boundary.
mod control;
mod observation;
mod retirement;
pub use observation::PacketFailure;
mod packet;
mod route;
mod split;
pub use packet::{PacketError, PacketPid};
pub use route::{EndpointRoute, TransactionTranslator, TransferType};
mod registers;
use registers::*;

/// The machine adapter owns the one register window and orders peripheral I/O.
/// Its microsecond clock must advance independently of this controller.
pub trait RegisterIo {
    fn read(&mut self, offset: usize) -> u32;
    fn write(&mut self, offset: usize, value: u32);
    fn now_us(&mut self) -> u32;
    /// Optional bounded observation; must not access FIFO or mutate custody.
    fn packet_failure(&mut self, _failure: PacketFailure) {}
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    AlreadyPrepared,
    UnsupportedCore,
    UnsupportedHardware,
    Timeout,
    WorkPressure,
    Overcurrent,
    PortNotEnabled,
}
impl Refusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyPrepared => "dwc2-already-prepared",
            Self::UnsupportedCore => "dwc2-core-unsupported",
            Self::UnsupportedHardware => "dwc2-hardware-unsupported",
            Self::Timeout => "dwc2-deadline",
            Self::WorkPressure => "dwc2-work-pressure",
            Self::Overcurrent => "dwc2-overcurrent",
            Self::PortNotEnabled => "dwc2-port-not-enabled",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerFacts {
    pub core_id: u32,
    pub channels: u8,
    pub fifo_words: u16,
    pub root_port: u32,
}
pub struct Controller<I> {
    io: I,
    prepared: bool,
    quiescent: bool,
}
const POLL_LIMIT: usize = 4_000_000;
impl<I: RegisterIo> Controller<I> {
    pub fn new(io: I) -> Self {
        Self {
            io,
            prepared: false,
            quiescent: true,
        }
    }

    /// Prepare polling/PIO machinery with DMA and interrupts disabled. Powering
    /// this root port is not downstream hub port-power or input acceptance.
    pub fn prepare(&mut self) -> Result<ControllerFacts, Refusal> {
        if self.prepared {
            return Err(Refusal::AlreadyPrepared);
        }
        let core_id = self.io.read(CORE_ID);
        if core_id & 0xffff_f000 != 0x4f54_2000 {
            return Err(Refusal::UnsupportedCore);
        }
        let result = self.prepare_core(core_id);
        if result.is_err() {
            // No DMA or transfer has been submitted. Keep interrupts masked,
            // remove bus power and release reset on every preparation failure.
            self.mask_interrupts();
            let port = self.io.read(PORT);
            self.io
                .write(PORT, port_write(port) & !(PORT_POWER | PORT_RESET));
        }
        result
    }
    fn prepare_core(&mut self, core_id: u32) -> Result<ControllerFacts, Refusal> {
        self.mask_interrupts();
        let usb = self.io.read(USB)
            & !((1 << 30)
                | (1 << 20)
                | (1 << 22)
                | (1 << 9)
                | (1 << 8)
                | (1 << 6)
                | (1 << 4)
                | (1 << 3));
        self.io.write(USB, usb | (1 << 29)); // Internal 8-bit UTMI+, host role.
        self.wait(RESET, AHB_IDLE, AHB_IDLE, 100_000)?;
        self.io.write(RESET, SOFT_RESET);
        self.wait(RESET, SOFT_RESET, 0, 100_000)?;
        self.delay(100_000)?;
        self.wait(INTERRUPT_STATUS, 1, 1, 100_000)?;
        let hardware = self.io.read(HARDWARE_2);
        let channels = (((hardware >> 14) & 15) + 1) as u8;
        let fifo_words = (self.io.read(HARDWARE_3) >> 16) as u16;
        if (hardware >> 3) & 3 != 2 || fifo_words < 1024 {
            return Err(Refusal::UnsupportedHardware);
        }
        self.io.write(CLOCK_GATE, 0);
        let host_config = self.io.read(HOST_CONFIG) & !3;
        self.io.write(HOST_CONFIG, host_config); // 30/60 MHz PHY host clock.
        self.io.write(RX_FIFO_SIZE, 512);
        self.io.write(NONPERIODIC_FIFO_SIZE, (256 << 16) | 512);
        self.io.write(PERIODIC_FIFO_SIZE, (256 << 16) | 768);
        self.io.write(RESET, (1 << 5) | (0x10 << 6));
        self.wait(RESET, 1 << 5, 0, 100_000)?;
        self.io.write(RESET, 1 << 4);
        self.wait(RESET, 1 << 4, 0, 100_000)?;
        for channel in 0..usize::from(channels) {
            self.io.write(0x50c + channel * 0x20, 0);
            self.io.write(0x508 + channel * 0x20, 0x7ff);
        }
        self.io.write(HOST_INTERRUPT_MASK, 0);
        self.io.write(INTERRUPT_STATUS, u32::MAX);
        let port = self.io.read(PORT);
        if port & OVERCURRENT != 0 {
            return Err(Refusal::Overcurrent);
        }
        self.io.write(PORT, port_write(port) | PORT_POWER);
        // The controller can be prepared without an attached device. Actual
        // attachment is retained as a fact, never inferred from successful reset.
        match self.wait(PORT, CONNECTED, CONNECTED, 500_000) {
            Ok(()) => {
                self.delay(100_000)?;
                let port = self.io.read(PORT);
                if port & OVERCURRENT != 0 {
                    return self.overcurrent();
                }
                self.io.write(PORT, port_write(port) | PORT_RESET);
                self.delay(50_000)?;
                let port = self.io.read(PORT);
                self.io.write(PORT, port_write(port) & !PORT_RESET);
                self.delay(20_000)?;
            }
            Err(Refusal::Timeout) => {}
            Err(error) => return Err(error),
        }
        let root_port = self.io.read(PORT);
        if root_port & OVERCURRENT != 0 {
            return self.overcurrent();
        }
        if root_port & CONNECTED != 0 && root_port & ENABLED == 0 {
            return Err(Refusal::PortNotEnabled);
        }
        if root_port & ENABLED != 0 {
            // Internal 8-bit UTMI PHY is 60 MHz: HS microframe / FS frame.
            let interval = if root_port & (3 << 17) == 0 {
                7499
            } else {
                59999
            };
            self.io.write(0x404, interval);
        }
        self.prepared = true;
        Ok(ControllerFacts {
            core_id,
            channels,
            fifo_words,
            root_port,
        })
    }
    pub fn is_prepared(&self) -> bool {
        self.prepared
    }
    /// True only while no packet was submitted, or its hardware halt was
    /// acknowledged. Software revocation and a timeout cannot manufacture it.
    pub fn is_quiescent(&self) -> bool {
        self.quiescent
    }
    fn mask_interrupts(&mut self) {
        self.io.write(INTERRUPT_MASK, 0);
        let ahb = self.io.read(AHB) & !((1 << 5) | 1);
        self.io.write(AHB, ahb);
    }
    fn overcurrent<T>(&mut self) -> Result<T, Refusal> {
        let port = self.io.read(PORT);
        self.io.write(PORT, port_write(port) & !PORT_POWER);
        Err(Refusal::Overcurrent)
    }
    fn wait(
        &mut self,
        offset: usize,
        mask: u32,
        expected: u32,
        timeout: u32,
    ) -> Result<(), Refusal> {
        let start = self.io.now_us();
        for _ in 0..POLL_LIMIT {
            if self.io.read(offset) & mask == expected {
                return Ok(());
            }
            if self.io.now_us().wrapping_sub(start) >= timeout {
                return Err(Refusal::Timeout);
            }
        }
        Err(Refusal::WorkPressure)
    }
    fn delay(&mut self, duration: u32) -> Result<(), Refusal> {
        let start = self.io.now_us();
        for _ in 0..POLL_LIMIT {
            if self.io.now_us().wrapping_sub(start) >= duration {
                return Ok(());
            }
        }
        Err(Refusal::WorkPressure)
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_io;
