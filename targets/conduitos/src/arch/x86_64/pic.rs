use super::io::{inb, outb, wait};

const MASTER_COMMAND: u16 = 0x20;
const MASTER_DATA: u16 = 0x21;
const SLAVE_COMMAND: u16 = 0xa0;
const SLAVE_DATA: u16 = 0xa1;

pub(super) fn initialize() {
    unsafe {
        outb(MASTER_COMMAND, 0x11);
        wait();
        outb(SLAVE_COMMAND, 0x11);
        wait();
        outb(MASTER_DATA, 0x20);
        wait();
        outb(SLAVE_DATA, 0x28);
        wait();
        outb(MASTER_DATA, 0x04);
        wait();
        outb(SLAVE_DATA, 0x02);
        wait();
        outb(MASTER_DATA, 0x01);
        wait();
        outb(SLAVE_DATA, 0x01);
        wait();
        outb(MASTER_DATA, 0xff);
        outb(SLAVE_DATA, 0xff);
    }
}

pub(super) fn unmask_timer() {
    let mask = unsafe { inb(MASTER_DATA) };
    unsafe { outb(MASTER_DATA, mask & !1) };
}

pub(super) fn mask_timer() {
    let mask = unsafe { inb(MASTER_DATA) };
    unsafe { outb(MASTER_DATA, mask | 1) };
}

pub(super) fn end_timer_interrupt() {
    unsafe { outb(MASTER_COMMAND, 0x20) };
}

/// Consume only an old IRQ0 edge while the PIT has no loaded counter.
/// Caller masks CPU interrupts; other PIC lines are masked only for this poll.
pub(super) fn discard_timer_pending() {
    unsafe {
        let previous = inb(MASTER_DATA);
        outb(MASTER_DATA, 0xfe);
        outb(MASTER_COMMAND, 0x0c);
        if inb(MASTER_COMMAND) == 0x80 {
            outb(MASTER_COMMAND, 0x60); // Specific EOI for IRQ0.
        }
        outb(MASTER_DATA, previous);
    }
}
