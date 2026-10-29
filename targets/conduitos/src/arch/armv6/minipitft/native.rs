//! BCM2835 mapping and exclusive pin routing, without ST7789 protocol knowledge.
use crate::{
    digital_base::bcm2835::{Direction, Pin, Registers as DigitalRegisters},
    spi_base::{
        SpiDisposition, SpiProvider, SpiTransaction,
        bcm2835::{Configuration, Controller, Registers},
    },
};
const GPIO: usize = 0x2020_0000;
const SPI0: usize = 0x2020_4000;
const SPI_PINS_0: u32 = 7 << 24; // GPIO8 / CE0
const SPI_PINS_1: u32 = 7 | (7 << 3); // GPIO10 / MOSI, GPIO11 / SCLK
const ATTACHMENT_PINS_2: u32 = (7 << 6) | (7 << 9) | (7 << 12) | (7 << 15); // GPIO22..25

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionRefusal {
    InterruptsEnabled,
    BoardUnavailable,
    WrongBoard,
    PinsBusy,
    SpiBusy,
    InvalidResource,
}

pub struct GpioRegisters {
    _owned: (),
}
struct SpiRegisters;
struct SpiPins;

// SAFETY: constructed only by admit after the Root's exclusive pin claim;
// admission is serialized and the mapped BCM2835 device window is ordered.
unsafe impl DigitalRegisters for GpioRegisters {
    fn read(&mut self, offset: usize) -> u32 {
        read(GPIO + offset)
    }
    fn write(&mut self, offset: usize, value: u32) {
        write(GPIO + offset, value);
    }
}
// SAFETY: admit retains the sole SPI0 controller and its exact routed pins.
unsafe impl Registers for SpiRegisters {
    fn read(&mut self, offset: usize) -> u32 {
        read(SPI0 + offset)
    }
    fn write(&mut self, offset: usize, value: u32) {
        write(SPI0 + offset, value);
    }
}

/// Fields drop in declaration order: stop SPI before releasing its pin route.
pub struct NativeSpi {
    controller: Controller<SpiRegisters>,
    _pins: SpiPins,
}
impl SpiProvider for NativeSpi {
    fn exchange(
        &mut self,
        request: &SpiTransaction<'_>,
        input: &mut [u8],
    ) -> Result<(), SpiDisposition> {
        SpiProvider::exchange(&mut self.controller, request, input)
    }
    fn revoke(&mut self) {
        self.controller.revoke();
    }
}
impl Drop for SpiPins {
    fn drop(&mut self) {
        // Clear only our fields; preserve unrelated functions. If ownership has
        // changed unexpectedly, leave the unknown owner's routing untouched.
        if read(SPI0) & (1 << 7) != 0 {
            return;
        }
        let first = read(GPIO);
        let second = read(GPIO + 4);
        if first & SPI_PINS_0 == 4 << 24 && second & SPI_PINS_1 == 4 | (4 << 3) {
            write(GPIO, first & !SPI_PINS_0);
            write(GPIO + 4, second & !SPI_PINS_1);
        }
    }
}

/// Possession, not a published runtime offer or a peripheral compatibility claim.
/// Each contact can be moved into its separately identified ReadyDigitalBase.
pub struct MiniPiTftResources {
    pub backlight: Pin<GpioRegisters>,
    pub command_data: Pin<GpioRegisters>,
    pub spi: NativeSpi,
    pub button_a: Pin<GpioRegisters>,
    pub button_b: Pin<GpioRegisters>,
    pub firmware_revision: u32,
}

/// Admit exactly the physical profile selected by trusted native Root.
///
/// # Safety
/// Root must possess SPI0 and GPIO8/10/11/22/23/24/25 exclusively, independently
/// approve the attached Adafruit product4484 electrical profile, retain BCM2835
/// device mappings with MMU/cache state suitable for MMIO, and serialize every
/// GPIO function-select change through teardown. No firmware, DMA, interrupt or
/// other provider may use these resources. Call once with IRQs disabled; board
/// recognition neither proves attachment presence nor supplies resource authority.
/// The image profile must fix the core clock at250MHz; divider64 then selects
/// 3.90625MHz, mode0, CE0, MSB first. MISO is unconnected and is not claimed.
pub unsafe fn admit() -> Result<MiniPiTftResources, AdmissionRefusal> {
    if super::super::interrupts_enabled() {
        return Err(AdmissionRefusal::InterruptsEnabled);
    }
    let revision =
        super::super::firmware_board_revision().ok_or(AdmissionRefusal::BoardUnavailable)?;
    if !super::is_model_b_plus_v1_2(revision) {
        return Err(AdmissionRefusal::WrongBoard);
    }
    let first = read(GPIO);
    let second = read(GPIO + 4);
    let third = read(GPIO + 8);
    if first & SPI_PINS_0 != 0 || second & SPI_PINS_1 != 0 || third & ATTACHMENT_PINS_2 != 0 {
        return Err(AdmissionRefusal::PinsBusy);
    }
    if read(SPI0) & (1 << 7) != 0 {
        return Err(AdmissionRefusal::SpiBusy);
    }
    let controller = Controller::admit(
        SpiRegisters,
        Configuration {
            chip_select: 0,
            clock_polarity: false,
            clock_phase: false,
            clock_divider: 64,
            maximum_bytes: 512,
            maximum_polls: 1_000_000,
        },
    )
    .map_err(|_| AdmissionRefusal::InvalidResource)?;
    // Inactive controller and preflight complete before any physical routing.
    write(GPIO, first | (4 << 24));
    write(GPIO + 4, second | 4 | (4 << 3));
    let spi = NativeSpi {
        controller,
        _pins: SpiPins,
    };
    let output = Direction::Output {
        initial_high: false,
        safe_high: false,
    };
    let backlight = Pin::admit(GpioRegisters { _owned: () }, 22, output)
        .map_err(|_| AdmissionRefusal::PinsBusy)?;
    let command_data = Pin::admit(GpioRegisters { _owned: () }, 25, output)
        .map_err(|_| AdmissionRefusal::PinsBusy)?;
    let button_a = Pin::admit(GpioRegisters { _owned: () }, 23, Direction::Input)
        .map_err(|_| AdmissionRefusal::PinsBusy)?;
    let button_b = Pin::admit(GpioRegisters { _owned: () }, 24, Direction::Input)
        .map_err(|_| AdmissionRefusal::PinsBusy)?;
    Ok(MiniPiTftResources {
        backlight,
        command_data,
        spi,
        button_a,
        button_b,
        firmware_revision: revision,
    })
}

fn barrier() {
    // ARM1176 device ordering: drain the write buffer across peripheral blocks.
    unsafe {
        core::arch::asm!("mcr p15, 0, {zero}, c7, c10, 4",zero=in(reg)0u32,options(nostack,preserves_flags));
    }
}
fn read(address: usize) -> u32 {
    barrier();
    let value = unsafe { core::ptr::read_volatile(address as *const u32) };
    barrier();
    value
}
fn write(address: usize, value: u32) {
    barrier();
    unsafe { core::ptr::write_volatile(address as *mut u32, value) };
    barrier();
}
