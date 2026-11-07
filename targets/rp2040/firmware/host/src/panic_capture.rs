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
        "CONDUIT_PANIC@1 heap_used={used} heap_peak={peak} heap_limit={} {info}",
        crate::config::HEAP_BYTES
    );
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    loop {
        cortex_m::asm::nop();
    }
}
