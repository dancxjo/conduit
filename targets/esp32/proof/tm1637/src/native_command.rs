//! Finite, read-only Root management framing. No request contains authority.
pub const MAXIMUM_REQUEST_BYTES: usize = 64;
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Offer,
    SoftwareOffer,
    Unsupported,
    TooLarge,
}
pub struct CommandBuffer {
    bytes: [u8; MAXIMUM_REQUEST_BYTES],
    used: usize,
    overflow: bool,
}
impl CommandBuffer {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAXIMUM_REQUEST_BYTES],
            used: 0,
            overflow: false,
        }
    }
    pub fn push(&mut self, byte: u8) -> Option<Command> {
        if byte == b'\n' {
            let command = if self.overflow {
                Command::TooLarge
            } else {
                let bytes = self.bytes[..self.used]
                    .strip_suffix(b"\r")
                    .unwrap_or(&self.bytes[..self.used]);
                if bytes == b"offer" {
                    Command::Offer
                } else if bytes == b"software-offer" {
                    Command::SoftwareOffer
                } else {
                    Command::Unsupported
                }
            };
            self.bytes.fill(0);
            self.used = 0;
            self.overflow = false;
            return Some(command);
        }
        if self.used == self.bytes.len() {
            self.overflow = true;
        } else {
            self.bytes[self.used] = byte;
            self.used += 1;
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_requests_refuse_overflow_and_never_infer_execution_permission() {
        let mut buffer = CommandBuffer::new();
        for byte in b"offer\r" {
            assert_eq!(buffer.push(*byte), None);
        }
        assert_eq!(buffer.push(b'\n'), Some(Command::Offer));
        for byte in b"install-native-plan" {
            assert_eq!(buffer.push(*byte), None);
        }
        assert_eq!(buffer.push(b'\n'), Some(Command::Unsupported));
        for byte in b"software-offer\n" {
            if *byte == b'\n' {
                assert_eq!(buffer.push(*byte), Some(Command::SoftwareOffer));
            } else {
                assert_eq!(buffer.push(*byte), None);
            }
        }
        for _ in 0..MAXIMUM_REQUEST_BYTES + 1 {
            assert_eq!(buffer.push(b'x'), None);
        }
        assert_eq!(buffer.push(b'\n'), Some(Command::TooLarge));
        for byte in b"offer" {
            assert_eq!(buffer.push(*byte), None);
        }
        assert_eq!(buffer.push(b'\n'), Some(Command::Offer));
        assert_eq!(buffer.push(b'\n'), Some(Command::Unsupported));
    }
}
