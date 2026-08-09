use core::arch::asm;

#[inline]
pub(super) unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nostack, nomem, preserves_flags)
        );
    }
}

#[inline]
pub(super) unsafe fn outl(port: u16, value: u32) {
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") port,
            in("eax") value,
            options(nostack, nomem, preserves_flags)
        );
    }
}

#[inline]
pub(super) unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nostack, nomem, preserves_flags)
        );
    }
    value
}

pub(super) unsafe fn wait() {
    unsafe { outb(0x80, 0) };
}
