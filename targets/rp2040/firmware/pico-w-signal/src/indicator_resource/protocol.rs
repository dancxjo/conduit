//! Private Pico indicator peripheral protocol, not Conduit Line framing.
//!
//! Every frame is exactly 96 bytes: `CIR1`, opcode, Boolean, two zero bytes,
//! acquisition nonce[16], device boot[16], build digest[32], request LE u64,
//! Play correlation[16]. HELLO=1 has only a nonzero nonce; READY=2 fills boot
//! and build. SET=3 must match all acquired identities; ACK=4 echoes SET.
//! At most eight SETs, one Play, request IDs 0..7. No retry or resynchronization.

pub const BYTES: usize = 96;
pub type Frame = [u8; BYTES];

pub struct Session {
    boot: [u8; 16],
    build: [u8; 32],
    nonce: Option<[u8; 16]>,
    play: Option<[u8; 16]>,
    next: u64,
    failed: bool,
}

pub enum Command {
    Ready(Frame),
    Set { state: bool, acknowledgment: Frame },
}

impl Session {
    pub fn new(boot: [u8; 16], build: [u8; 32]) -> Self {
        Self {
            boot,
            build,
            nonce: None,
            play: None,
            next: 0,
            failed: false,
        }
    }

    /// Decode before the effect. The caller sends ACK only after GPIO completes.
    /// Any refusal permanently poisons this acquisition until USB reconnects.
    pub fn accept(&mut self, frame: Frame) -> Option<Command> {
        let result = self.decode(frame);
        if result.is_none() {
            self.failed = true;
        }
        result
    }

    fn decode(&mut self, mut frame: Frame) -> Option<Command> {
        if self.failed || &frame[..4] != b"CIR1" || frame[6..8] != [0; 2] {
            return None;
        }
        let nonce: [u8; 16] = frame[8..24].try_into().ok()?;
        match frame[4] {
            1 if self.nonce.is_none()
                && frame[5] == 0
                && nonce != [0; 16]
                && frame[24..].iter().all(|b| *b == 0) =>
            {
                self.nonce = Some(nonce);
                frame[4] = 2;
                frame[24..40].copy_from_slice(&self.boot);
                frame[40..72].copy_from_slice(&self.build);
                Some(Command::Ready(frame))
            }
            3 if self.nonce == Some(nonce)
                && frame[5] <= 1
                && frame[24..40] == self.boot
                && frame[40..72] == self.build
                && self.next < 8 =>
            {
                let request = u64::from_le_bytes(frame[72..80].try_into().ok()?);
                let play: [u8; 16] = frame[80..96].try_into().ok()?;
                if request != self.next
                    || play == [0; 16]
                    || self.play.is_some_and(|expected| expected != play)
                {
                    return None;
                }
                self.play = Some(play);
                self.next += 1;
                frame[4] = 4;
                Some(Command::Set {
                    state: frame[5] == 1,
                    acknowledgment: frame,
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acquired() -> (Session, Frame) {
        let mut session = Session::new([2; 16], [3; 32]);
        let mut hello = [0; BYTES];
        hello[..4].copy_from_slice(b"CIR1");
        hello[4] = 1;
        hello[8..24].fill(1);
        let Some(Command::Ready(mut ready)) = session.accept(hello) else {
            panic!()
        };
        assert_eq!(&ready[24..40], &[2; 16]);
        assert_eq!(&ready[40..72], &[3; 32]);
        ready[4] = 3;
        ready[80..96].fill(4);
        (session, ready)
    }

    #[test]
    fn eight_exact_effects_then_bound_refusal() {
        let (mut session, mut frame) = acquired();
        for request in 0_u64..8 {
            frame[72..80].copy_from_slice(&request.to_le_bytes());
            frame[5] = (request % 2 == 0) as u8;
            let Some(Command::Set {
                state,
                acknowledgment,
            }) = session.accept(frame)
            else {
                panic!()
            };
            assert_eq!(state, frame[5] == 1);
            let mut expected = frame;
            expected[4] = 4;
            assert_eq!(acknowledgment, expected);
        }
        frame[72..80].copy_from_slice(&8_u64.to_le_bytes());
        assert!(session.accept(frame).is_none());
    }

    #[test]
    fn corrupt_identity_header_or_request_poison_acquisition() {
        for byte in (0..5).chain(6..80) {
            let (mut session, frame) = acquired();
            let mut wrong = frame;
            wrong[byte] ^= 0x80;
            assert!(session.accept(wrong).is_none(), "byte {byte}");
            assert!(session.accept(frame).is_none());
        }
        let (mut session, mut frame) = acquired();
        frame[5] = 2;
        assert!(session.accept(frame).is_none());
    }

    #[test]
    fn replay_and_different_play_refuse() {
        for different_play in [false, true] {
            let (mut session, mut frame) = acquired();
            assert!(session.accept(frame).is_some());
            if different_play {
                frame[72..80].copy_from_slice(&1_u64.to_le_bytes());
                frame[80] ^= 1;
            }
            assert!(session.accept(frame).is_none());
        }
    }
}
