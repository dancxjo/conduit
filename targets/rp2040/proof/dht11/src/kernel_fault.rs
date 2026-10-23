//! Retained bench fault receipt across a ROM BOOTSEL reset. Reflashing the same
//! image reports the fault and waits for lifecycle control, without retrying it.
use core::mem::MaybeUninit;
const MAGIC: u32 = 0x43444631;
#[link_section = ".uninit.kernel_fault"]
static mut LAST: MaybeUninit<[u32; 9]> = MaybeUninit::uninit();
pub fn record(
    kind: u32,
    peak: usize,
    used: usize,
    requested: usize,
    alignment: usize,
    line: u32,
    column: u32,
) {
    let mut words = [
        MAGIC,
        kind,
        peak as u32,
        used as u32,
        requested as u32,
        alignment as u32,
        line,
        column,
        0,
    ];
    words[8] = words[..8].iter().fold(0, |a, b| a ^ b);
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(LAST).cast::<[u32; 9]>(), words);
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}
pub fn take() -> Option<[u32; 9]> {
    let words = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(LAST).cast::<[u32; 9]>()) };
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(LAST).cast::<u32>(), 0);
    }
    (words[0] == MAGIC && words[..8].iter().fold(0, |a, b| a ^ b) == words[8]).then_some(words)
}
