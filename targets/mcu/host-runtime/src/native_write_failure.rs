//! TM1637 HostCallFailed detail tags, descriptive only. No retry or authority.
//! 0x0100 | packet << 4 | byte identifies zero-based NACK location. The wire
//! admits at most three packets of seven bytes: packet <= 2 and byte <= 6.
//! Unexpected indices map to 0x01ff explicitly, never truncate into a valid
//! location. Other tags retain the refusal class without inventing a cause for
//! an opaque provider error. The outer FailureCode remains HostCallFailed.
use conduit_tm1637_wire::{owner::WriteCallRefusal, Failure};

pub const NACK: u16 = 0x0100;
pub const NACK_LOCATION_UNREPRESENTABLE: u16 = 0x01ff;
pub const CANCELLED: u16 = 0x0200;
pub const PROVIDER: u16 = 0x0300;
pub const INVALID_GEOMETRY: u16 = 0x0400;
pub const PACKETS: u16 = 0x0500;
pub const STALE_REQUEST: u16 = 0x0600;
pub const SEQUENCE_EXHAUSTED: u16 = 0x0700;

fn nack_location(packet: u8, byte: u8) -> u16 {
    if packet <= 2 && byte <= 6 {
        NACK | (u16::from(packet) << 4) | u16::from(byte)
    } else {
        NACK_LOCATION_UNREPRESENTABLE
    }
}
pub(super) fn detail<E>(reason: &WriteCallRefusal<E>) -> u16 {
    match reason {
        WriteCallRefusal::Electrical(Failure::Nack { packet, byte }) => {
            nack_location(*packet, *byte)
        }
        WriteCallRefusal::Electrical(Failure::Cancelled) => CANCELLED,
        WriteCallRefusal::Electrical(Failure::Provider(_)) => PROVIDER,
        WriteCallRefusal::Electrical(Failure::InvalidGeometry) => INVALID_GEOMETRY,
        WriteCallRefusal::Packets(_) => PACKETS,
        WriteCallRefusal::StaleRequest => STALE_REQUEST,
        WriteCallRefusal::SequenceExhausted => SEQUENCE_EXHAUSTED,
        // These continue through the existing Denied branch, not this helper.
        WriteCallRefusal::Capability(_)
        | WriteCallRefusal::Possession
        | WriteCallRefusal::WrongBinding => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_admitted_nack_location_is_unique_and_out_of_bounds_is_explicit() {
        let mut seen = [false; 0x27];
        for packet in 0..3 {
            for byte in 0..7 {
                let detail = nack_location(packet, byte);
                let location = usize::from(detail - NACK);
                assert!(!seen[location]);
                seen[location] = true;
                assert_ne!(detail, NACK_LOCATION_UNREPRESENTABLE);
            }
        }
        for (packet, byte) in [(3, 0), (0, 7), (16, 0), (255, 255)] {
            assert_eq!(nack_location(packet, byte), NACK_LOCATION_UNREPRESENTABLE);
        }
    }
    #[test]
    fn cancellation_provider_and_geometry_are_distinct() {
        let cancelled = detail::<()>(&WriteCallRefusal::Electrical(Failure::Cancelled));
        let provider = detail(&WriteCallRefusal::Electrical(Failure::Provider(())));
        let geometry = detail::<()>(&WriteCallRefusal::Electrical(Failure::InvalidGeometry));
        assert_eq!((cancelled, provider, geometry), (0x200, 0x300, 0x400));
    }
}
