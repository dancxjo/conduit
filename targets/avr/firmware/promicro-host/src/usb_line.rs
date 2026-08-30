use atmega_hal::{pac, usb::AvrGenericUsbBus};
use core::mem::MaybeUninit;
use usb_device::{
    bus::UsbBusAllocator,
    descriptor::lang_id::LangID,
    device::{StringDescriptors, UsbDevice, UsbDeviceBuilder, UsbVidPid},
};
use usbd_serial::{CdcAcmClass, USB_CLASS_CDC};

type Bus = AvrGenericUsbBus<pac::PLL>;

pub struct UsbLine {
    device: UsbDevice<'static, Bus>,
    serial: CdcAcmClass<'static, Bus>,
}

impl UsbLine {
    /// Initialize the ATmega32U4 USB CDC mechanism used as the Host Line.
    ///
    /// The single static allocator is initialized exactly once after ownership
    /// of the USB peripheral and PLL has been acquired.
    pub fn new(usb: pac::USB_DEVICE, pll: pac::PLL, boot_serial: &'static str) -> Self {
        configure_pll(&pll);
        static mut BUS: MaybeUninit<UsbBusAllocator<Bus>> = MaybeUninit::uninit();
        // SAFETY: `main` calls this once after taking the unique peripherals,
        // before interrupts are enabled. `BUS` then remains initialized for
        // the entire program and is only exposed through this owned Line.
        let bus = unsafe {
            BUS.write(AvrGenericUsbBus::with_suspend_notifier(usb, pll));
            &*BUS.as_ptr()
        };
        // The Host protocol is already finitely framed, so use the lower-level
        // packet CDC class instead of adding another pair of stream buffers.
        let serial = CdcAcmClass::new(bus, 64);
        let strings = StringDescriptors::new(LangID::EN).serial_number(boot_serial);
        let device = UsbDeviceBuilder::new(bus, UsbVidPid(0x1b4f, 0x9206))
            .strings(&[strings])
            .unwrap()
            .device_class(USB_CLASS_CDC)
            .build();
        Self { device, serial }
    }

    pub fn poll(&mut self) -> bool {
        self.device.poll(&mut [&mut self.serial])
    }

    pub fn read(&mut self, bytes: &mut [u8]) -> usb_device::Result<usize> {
        self.serial.read_packet(bytes)
    }

    pub fn write(&mut self, bytes: &[u8]) -> usb_device::Result<usize> {
        self.serial.write_packet(bytes)
    }
}

fn configure_pll(pll: &pac::PLL) {
    pll.pllcsr.write(|write| write.pindiv().set_bit());
    pll.pllfrq.write(|write| {
        write
            .pdiv()
            .mhz96()
            .plltm()
            .factor_15()
            .pllusb()
            .set_bit()
    });
    pll.pllcsr.modify(|_, write| write.plle().set_bit());
    while pll.pllcsr.read().plock().bit_is_clear() {}
}
