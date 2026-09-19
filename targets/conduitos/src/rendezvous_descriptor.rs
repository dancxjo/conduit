//! ConduitOS consumption of the shared target-neutral rendezvous bytes.

use conduit_body::{
    RendezvousCborRefusal, RendezvousLineFamily, decode_running_host_rendezvous_cbor,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendezvousDescriptorReceipt {
    pub candidate_count: u8,
    pub line_family_mask: u8,
}

/// Inspect an admitted caller-owned buffer without allocation or retaining its
/// capability. Actual Line attachment must independently admit a matching Base.
pub fn inspect(
    encoded: &[u8],
    now_millis: u64,
) -> Result<RendezvousDescriptorReceipt, RendezvousCborRefusal> {
    let descriptor = decode_running_host_rendezvous_cbor(encoded, now_millis)?;
    let mut line_family_mask = 0;
    for candidate in descriptor.candidates() {
        line_family_mask |= 1 << family_code(candidate.line_family);
    }
    Ok(RendezvousDescriptorReceipt {
        candidate_count: descriptor.candidates().len() as u8,
        line_family_mask,
    })
}

const fn family_code(family: RendezvousLineFamily) -> u8 {
    match family {
        RendezvousLineFamily::AuthenticatedTlsStream => 0,
        RendezvousLineFamily::AuthenticatedConduitLine => 1,
        RendezvousLineFamily::LocalLoopbackWebSocket => 2,
        RendezvousLineFamily::AttendedSerial => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn fixture() -> Vec<u8> {
        include_str!("../../../architecture/body/schemas/running-host-rendezvous-v1.hex")
            .trim()
            .as_bytes()
            .chunks_exact(2)
            .map(|digits| {
                let digit = |value| match value {
                    b'0'..=b'9' => value - b'0',
                    b'a'..=b'f' => value - b'a' + 10,
                    _ => panic!("fixture is lowercase hexadecimal"),
                };
                (digit(digits[0]) << 4) | digit(digits[1])
            })
            .collect()
    }

    #[test]
    fn conduitos_consumes_the_same_checked_descriptor_without_claiming_a_line() {
        assert_eq!(
            inspect(&fixture(), 1_700_000_000_000),
            Ok(RendezvousDescriptorReceipt {
                candidate_count: 2,
                line_family_mask: 0b0011,
            })
        );
    }
}
