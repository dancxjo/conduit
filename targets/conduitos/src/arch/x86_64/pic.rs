use super::io::{inb, outb, wait};
use core::sync::atomic::{AtomicU8, Ordering};

// Unknown until this owner initializes and explicitly selects IRR.
static MASTER_READ_MODE: AtomicU8 = AtomicU8::new(0);

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
        outb(MASTER_COMMAND, 0x0a);
        MASTER_READ_MODE.store(0x0a, Ordering::Release);
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

/// Observe IRQ0 without acknowledging it, changing masks, or touching cascade.
/// Caller holds the single-CPU interrupt mask. All master OCW3 read-mode writes
/// are owned here; poll0x0c does not select a new read register (RR bit is0).
/// Unknown initialization refuses, rather than assuming no pending interrupt.
pub(super) fn timer_quiescent() -> bool {
    let previous = MASTER_READ_MODE.load(Ordering::Acquire);
    if !matches!(previous, 0x0a | 0x0b) {
        return false;
    }
    let (irr, isr) = unsafe {
        outb(MASTER_COMMAND, 0x0a);
        let irr = inb(MASTER_COMMAND);
        outb(MASTER_COMMAND, 0x0b);
        let isr = inb(MASTER_COMMAND);
        outb(MASTER_COMMAND, previous);
        (irr, isr)
    };
    irq_zero_quiescent(irr, isr)
}
fn irq_zero_quiescent(irr: u8, isr: u8) -> bool {
    (irr | isr) & 1 == 0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn irq_zero_fence_rejects_pending_or_in_service_but_not_other_lines() {
        for irr in 0..=u8::MAX {
            for isr in 0..=u8::MAX {
                assert_eq!(irq_zero_quiescent(irr, isr), irr & 1 == 0 && isr & 1 == 0);
            }
        }
        assert!(irq_zero_quiescent(0b100, 0b100)); // RTC cascade is not consumed.
    }
}
