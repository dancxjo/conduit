//! PIT/PIC ownership transitions. All operations require CPU interrupts masked.

use super::{inb, outb, present};
use crate::machine::BaseError;

pub(super) fn stop() {
    // Channel 0's PC gate is held high. Mode 1 waits for a new gate edge, so
    // firmware periodic activity cannot recur. Write the complete count too:
    // QEMU does not reschedule its old IRQ timer on a control-word-only write.
    // The transition to mode 1 may itself raise OUT; quiesce acknowledges that
    // pre-arm edge before a new generation can become eligible.
    unsafe {
        // Retire only IRQ0. Root's independent domain-budget cascade may be
        // active when a Source timer completes during User execution.
        outb(0x21, inb(0x21) | 1);
        outb(0x43, 0x32);
        outb(0x40, 0);
        outb(0x40, 0);
    }
}

pub(super) fn quiesce() -> Result<(), BaseError> {
    stop();
    // Poll only IRQ0, then restore the other Root owners' mask bits. Poll provides
    // one bounded acknowledge of a pre-arm/retired IRQ, while the source is
    // stopped and the CPU cannot dispatch it into a fresh timer generation.
    let pending = unsafe {
        let mask = inb(0x21);
        outb(0x21, 0xfe);
        outb(0x20, 0x0c);
        let pending = inb(0x20);
        outb(0x21, mask | 1);
        pending
    };
    if pending & 0x80 != 0 {
        if pending & 7 != 0 {
            return Err(BaseError::Unavailable);
        }
        unsafe { outb(0x20, 0x60) };
        present(b"CONDUIT_IA32_TIMER_BOUNDARY {\"operation\":\"quiesce\",\"retired_irq0\":true}\n");
    }
    Ok(())
}

pub(super) fn start() {
    start_count(1193);
}

pub(super) fn start_count(ticks: u16) {
    unsafe {
        outb(0x43, 0x30);
        outb(0x40, ticks as u8);
        outb(0x40, (ticks >> 8) as u8);
        outb(0x21, inb(0x21) & !1);
    }
}

pub(super) fn report_inherited_state() {
    let (mode, pending) = unsafe {
        // 8254 read-back: latch status only, counter 0.
        outb(0x43, 0xe2);
        let mode = (inb(0x40) >> 1) & 7;
        outb(0x20, 0x0a);
        (mode, inb(0x20) & 1 != 0)
    };
    present(b"CONDUIT_IA32_TIMER_BOUNDARY {\"operation\":\"initialize\",\"inherited_pit_mode\":");
    present(&[b'0' + mode]);
    present(b",\"pending_irq0\":");
    present(if pending { b"true" } else { b"false" });
    present(b"}\n");
}
