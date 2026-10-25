//! Fixed firmware property packet for observing the actual UART reference rate.
//!
//! Protocol: https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface
//! (Get clock rate measured, tag 0x00030047; UART clock ID 2).
//! This codec performs no I/O and is not a clock owner or readiness provider.
//! Board integration must supply exclusive mailbox transport, a VC-addressable
//! aligned buffer, cache maintenance, barriers and exact channel/address reply
//! matching before parsing. A reported rate is a snapshot, not evidence the
//! clock is enabled or stable for the lifetime of EarlySerial.

const MEASURED_RATE: u32 = 0x0003_0047;
const UART_CLOCK: u32 = 2;
const SUCCESS: u32 = 0x8000_0000;

/// One tag, eight-byte value buffer, and end tag. Words use little-endian wire
/// representation. Alignment alone does not make this memory firmware-accessible.
#[repr(C, align(16))]
pub struct UartClockPacket {
    words: [u32; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockRefusal {
    MalformedResponse,
    FirmwareRefused,
    MissingRate,
}

impl Default for UartClockPacket {
    fn default() -> Self {
        Self::request()
    }
}

impl UartClockPacket {
    /// A fresh query; never reuses response flags or a previous reported rate.
    pub const fn request() -> Self {
        Self {
            words: [
                32u32.to_le(),
                0,
                MEASURED_RATE.to_le(),
                8u32.to_le(),
                0,
                UART_CLOCK.to_le(),
                0,
                0,
            ],
        }
    }

    /// Exact wire words for a transport-owned copy. Do not submit this borrowed
    /// memory asynchronously; firmware ownership must never overlap Rust access.
    pub fn wire_words(&self) -> &[u32; 8] {
        &self.words
    }

    /// Parse a completed, transport-validated reply copied into CPU-owned memory.
    /// An unchanged request, unsupported tag, partial reply, different clock/tag,
    /// modified buffer length, or nonzero terminator cannot supply a baud basis.
    pub fn measured_hz(reply: &[u32; 8]) -> Result<u32, ClockRefusal> {
        let w = reply.map(u32::from_le);
        if w[0] != 32
            || w[2] != MEASURED_RATE
            || w[3] != 8
            || w[4] != (SUCCESS | 8)
            || w[5] != UART_CLOCK
            || w[7] != 0
        {
            return Err(ClockRefusal::MalformedResponse);
        }
        if w[1] != SUCCESS {
            return Err(ClockRefusal::FirmwareRefused);
        }
        if w[6] == 0 {
            return Err(ClockRefusal::MissingRate);
        }
        Ok(w[6])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(rate: u32) -> [u32; 8] {
        [
            32,
            SUCCESS,
            MEASURED_RATE,
            8,
            SUCCESS | 8,
            UART_CLOCK,
            rate,
            0,
        ]
        .map(u32::to_le)
    }

    #[test]
    fn transport_layout_is_exact_and_aligned() {
        assert_eq!(core::mem::size_of::<UartClockPacket>(), 32);
        assert_eq!(core::mem::align_of::<UartClockPacket>(), 16);
        assert_eq!(
            UartClockPacket::request().wire_words().map(u32::from_le),
            [32, 0, MEASURED_RATE, 8, 0, UART_CLOCK, 0, 0]
        );
    }

    #[test]
    fn unprocessed_request_never_becomes_a_clock() {
        assert!(UartClockPacket::measured_hz(UartClockPacket::request().wire_words()).is_err());
    }

    #[test]
    fn reports_actual_rate_without_assuming_48mhz() {
        for rate in [3_000_000, 48_000_000, 47_999_999] {
            assert_eq!(UartClockPacket::measured_hz(&response(rate)), Ok(rate));
        }
        assert_eq!(
            UartClockPacket::measured_hz(&response(0)),
            Err(ClockRefusal::MissingRate)
        );
    }

    #[test]
    fn every_structural_word_is_checked() {
        for index in [0, 2, 3, 4, 5, 7] {
            for bit in 0..32 {
                let mut reply = response(3_000_000);
                reply[index] ^= (1u32 << bit).to_le();
                assert_eq!(
                    UartClockPacket::measured_hz(&reply),
                    Err(ClockRefusal::MalformedResponse),
                    "word {index}, bit {bit}"
                );
            }
        }
    }

    #[test]
    fn partial_and_reserved_firmware_codes_refuse() {
        for code in [0, SUCCESS | 1, u32::MAX] {
            let mut reply = response(3_000_000);
            reply[1] = code.to_le();
            assert_eq!(
                UartClockPacket::measured_hz(&reply),
                Err(ClockRefusal::FirmwareRefused)
            );
        }
    }
}
