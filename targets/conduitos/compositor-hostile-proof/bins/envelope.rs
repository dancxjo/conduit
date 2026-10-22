#![no_std]
#![no_main]
#[path = "../guest.rs"]
mod guest;
/// # Safety
/// Root must install the fixed service grants before entering this hostile fixture.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub unsafe extern "C" fn compositor_entry(exchange: *mut u8) -> ! {
    unsafe { guest::run(exchange, 0) }
}
