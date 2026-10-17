use super::io::outb;

const CONTROL: u16 = 0x43;
const CHANNEL_ZERO: u16 = 0x40;
const ONE_MILLISECOND_COUNT: u16 = 1_193;

pub(super) fn arm_one_shot() {
    unsafe {
        // Channel 0, low/high byte, mode 0 interrupt on terminal count.
        outb(CONTROL, 0x30);
        // The control word stops the old counter. Poll before loading the new
        // count so a firmware/previous edge cannot become the new timer wake.
        super::pic::discard_timer_pending();
        outb(CHANNEL_ZERO, ONE_MILLISECOND_COUNT as u8);
        outb(CHANNEL_ZERO, (ONE_MILLISECOND_COUNT >> 8) as u8);
    }
}
