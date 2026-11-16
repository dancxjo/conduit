//! Bounded slave-mode halt handshake, including receive-status retirement.
use super::{Controller, POLL_LIMIT, RegisterIo, packet::*, registers::*};
impl<I: RegisterIo> Controller<I> {
    /// Drain status entries while awaiting CHHLTD: in slave mode the core may
    /// assert that bit only after software pops the channel-halted RX status.
    /// Late data is discarded and reported; it cannot become a successful read.
    /// Returns whether unexpected receive residue was observed.
    pub(super) fn halt_packet(&mut self, characteristic: u32, periodic: bool) -> Result<bool, ()> {
        let start = self.io.now_us();
        let mut requested = false;
        let mut flushed = false;
        let mut unexpected = false;
        let queue = if periodic { 0x410 } else { TX_SPACE };
        for _ in 0..POLL_LIMIT {
            if !requested {
                // A halt needs a request-queue slot just like a transfer.
                // With a full queue, CHDIS alone flushes queued requests;
                // submit the halt once space becomes available, never data.
                let room = self.io.read(queue) & (0xff << 16) != 0;
                if room || !flushed {
                    self.io.write(
                        CHARACTER,
                        characteristic | HALT | if room { ENABLE } else { 0 },
                    );
                    flushed = true;
                }
                requested = room;
            }
            if self.io.read(INTERRUPT_STATUS) & (1 << 4) != 0 {
                let status = self.io.read(RX_POP);
                let count = (status >> 4) & 0x7ff;
                let kind = (status >> 17) & 15;
                for _ in 0..count.div_ceil(4) {
                    let _ = self.io.read(FIFO);
                }
                unexpected |= status & 15 != 0 || count != 0 || !matches!(kind, 3 | 5 | 7);
            } else if self.io.read(STATUS) & HALTED != 0 && self.io.read(CHARACTER) & ENABLE == 0 {
                return Ok(unexpected);
            }
            if self.io.now_us().wrapping_sub(start) >= 100_000 {
                return Err(());
            }
        }
        Err(())
    }
}
