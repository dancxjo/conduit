//! BCM2835 ownership, power and ordered MMIO adapter for class-neutral DWC2.
//! Hub/device/HID exchanges belong to the shared reviewed protocol plots.
use super::{GPIO_BASE, mailbox, read_counter, read32, write32};
use crate::arch::dwc2::{Controller, ControllerFacts, RegisterIo};
use core::sync::atomic::{AtomicBool, Ordering};
const USB_BASE: usize = 0x2098_0000;
static OWNED: AtomicBool = AtomicBool::new(false);
#[repr(C, align(16))]
struct PowerMessage([u32; 8]);
struct Registers;
impl RegisterIo for Registers {
    fn read(&mut self, offset: usize) -> u32 {
        debug_assert!((offset <= 0xe00 || offset == 0x1000) && offset & 3 == 0);
        unsafe { read32(USB_BASE + offset) }
    }
    fn write(&mut self, offset: usize, value: u32) {
        debug_assert!((offset <= 0xe00 || offset == 0x1000) && offset & 3 == 0);
        unsafe { write32(USB_BASE + offset, value) };
    }
    fn now_us(&mut self) -> u32 {
        read_counter() as u32
    }
}
/// Root retains sole register ownership. This type supplies no USB attachment,
/// endpoint claim or input provider merely because preparation succeeds.
pub struct UsbHost {
    controller: Controller<Registers>,
    facts: ControllerFacts,
}
impl UsbHost {
    pub fn prepare() -> Result<Self, &'static str> {
        if OWNED.swap(true, Ordering::AcqRel) {
            return Err("bcm2835-usb-already-owned");
        }
        let mut power = PowerMessage([32, 0, 0x28001, 8, 8, 3, 3, 0]);
        mailbox::call(&mut power.0)?;
        if power.0[4] != 0x8000_0008 || power.0[5] != 3 || power.0[6] & 3 != 1 {
            return Err("bcm2835-usb-power-refused");
        }
        // B+ LAN_RUN is internal board wiring, not a discovered device policy.
        // Establish the high latch before output mode to avoid a reset glitch.
        unsafe {
            write32(GPIO_BASE + 0x1c, 1 << 31);
            let select = read32(GPIO_BASE + 0x0c) & !(7 << 3);
            write32(GPIO_BASE + 0x0c, select | (1 << 3));
        }
        let mut controller = Controller::new(Registers);
        let facts = controller.prepare().map_err(|error| error.as_str())?;
        Ok(Self { controller, facts })
    }
    pub fn facts(&self) -> ControllerFacts {
        self.facts
    }
    pub fn is_prepared(&self) -> bool {
        self.controller.is_prepared()
    }
}
