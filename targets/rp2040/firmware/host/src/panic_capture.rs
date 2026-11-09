//! Bounded platform recovery, never an execution or recording acknowledgment.
//! The ROM recovery tool may inspect this SRAM after a panic. No flash is written.
use core::{
    fmt::{self, Write},
    mem::MaybeUninit,
    panic::PanicInfo,
};

const BYTES: usize = 512;
#[no_mangle]
#[link_section = ".uninit.CONDUIT_PANIC_CAPTURE"]
static mut CONDUIT_PANIC_CAPTURE: MaybeUninit<[u8; BYTES]> = MaybeUninit::uninit();

struct Bounded<'a> {
    bytes: &'a mut [u8],
    used: usize,
}
impl Write for Bounded<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let count = value.len().min(self.bytes.len().saturating_sub(self.used));
        self.bytes[self.used..self.used + count].copy_from_slice(&value.as_bytes()[..count]);
        self.used += count;
        if count == value.len() {
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    recover(format_args!("{info}"))
}

#[cortex_m_rt::exception]
unsafe fn HardFault(frame: &cortex_m_rt::ExceptionFrame) -> ! {
    recover(format_args!(
        "HardFault pc={:#010x} lr={:#010x} xpsr={:#010x} sp={:#010x}",
        frame.pc(),
        frame.lr(),
        frame.xpsr(),
        frame as *const _ as usize
    ))
}

fn recover(reason: fmt::Arguments<'_>) -> ! {
    cortex_m::interrupt::disable();
    // SAFETY: this is the terminal single-core panic path. Initialize every
    // byte before making a slice; no allocator or provider is borrowed here.
    let bytes = unsafe {
        let raw = core::ptr::addr_of_mut!(CONDUIT_PANIC_CAPTURE).cast::<u8>();
        core::ptr::write_bytes(raw, 0, BYTES);
        core::slice::from_raw_parts_mut(raw, BYTES)
    };
    let (used, peak) = crate::heap::observed_usage();
    let mut output = Bounded { bytes, used: 0 };
    let _ = write!(
        output,
        "CONDUIT_PANIC@1 heap_used={used} heap_peak={peak} heap_limit={} stack_spare={:?} stack_msp={:#010x} {reason}",
        crate::config::HEAP_BYTES,
        crate::stack_probe::spare_bytes(),
        cortex_m::register::msp::read()
    );
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    loop {
        cortex_m::asm::nop();
    }
}
