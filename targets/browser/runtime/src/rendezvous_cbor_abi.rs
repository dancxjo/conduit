//! Browser/WASM entrance for the shared bounded rendezvous descriptor bytes.

use std::cell::RefCell;

use conduit_body::{
    decode_running_host_rendezvous_cbor, RendezvousCborRefusal, RendezvousDescriptorRefusal,
    MAX_RENDEZVOUS_CBOR_BYTES,
};

const ERROR_BOUND: i32 = -1;
const ERROR_TRUNCATED: i32 = -2;
const ERROR_MALFORMED: i32 = -3;
const ERROR_NON_CANONICAL: i32 = -4;
const ERROR_DUPLICATE: i32 = -5;
const ERROR_MISSING: i32 = -6;
const ERROR_VERSION: i32 = -7;
const ERROR_MANDATORY: i32 = -8;
const ERROR_LINE_FAMILY: i32 = -9;
const ERROR_EXTENSION: i32 = -10;
const ERROR_EXPIRED: i32 = -11;
const ERROR_SEMANTIC: i32 = -12;

thread_local! {
    static INPUT: RefCell<[u8; MAX_RENDEZVOUS_CBOR_BYTES]> =
        const { RefCell::new([0; MAX_RENDEZVOUS_CBOR_BYTES]) };
}

#[no_mangle]
pub extern "C" fn conduit_browser_rendezvous_cbor_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_browser_rendezvous_cbor_input_capacity() -> usize {
    MAX_RENDEZVOUS_CBOR_BYTES
}

/// Decode one caller-supplied descriptor. A positive result is its candidate
/// count; the one-use capability is erased with the temporary decoded view.
#[no_mangle]
pub extern "C" fn conduit_browser_rendezvous_cbor_decode(
    length: usize,
    now_millis_high: u32,
    now_millis_low: u32,
) -> i32 {
    if length == 0 || length > MAX_RENDEZVOUS_CBOR_BYTES {
        return ERROR_BOUND;
    }
    let now_millis = (u64::from(now_millis_high) << 32) | u64::from(now_millis_low);
    INPUT.with(|input| {
        let input = input.borrow();
        let result = match decode_running_host_rendezvous_cbor(&input[..length], now_millis) {
            Ok(descriptor) => descriptor.candidates().len() as i32,
            Err(error) => error_code(error),
        };
        result
    })
}

fn error_code(error: RendezvousCborRefusal) -> i32 {
    match error {
        RendezvousCborRefusal::EncodedBound => ERROR_BOUND,
        RendezvousCborRefusal::Truncated => ERROR_TRUNCATED,
        RendezvousCborRefusal::Malformed => ERROR_MALFORMED,
        RendezvousCborRefusal::NonCanonical => ERROR_NON_CANONICAL,
        RendezvousCborRefusal::DuplicateField => ERROR_DUPLICATE,
        RendezvousCborRefusal::MissingField => ERROR_MISSING,
        RendezvousCborRefusal::UnsupportedVersion => ERROR_VERSION,
        RendezvousCborRefusal::UnsupportedMandatoryField => ERROR_MANDATORY,
        RendezvousCborRefusal::UnsupportedLineFamily => ERROR_LINE_FAMILY,
        RendezvousCborRefusal::ExtensionBound => ERROR_EXTENSION,
        RendezvousCborRefusal::Descriptor(RendezvousDescriptorRefusal::Expired) => ERROR_EXPIRED,
        RendezvousCborRefusal::Descriptor(_) => ERROR_SEMANTIC,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::vec::Vec<u8> {
        include_str!("../../../../architecture/body/schemas/running-host-rendezvous-v1.hex")
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
    fn browser_boundary_consumes_the_checked_canonical_vector() {
        let fixture = fixture();
        INPUT.with(|input| input.borrow_mut()[..fixture.len()].copy_from_slice(&fixture));
        let now = 1_700_000_000_000_u64;
        assert_eq!(
            conduit_browser_rendezvous_cbor_decode(fixture.len(), (now >> 32) as u32, now as u32,),
            2
        );
        INPUT.with(|input| {
            input.borrow_mut()[..fixture.len() - 1].copy_from_slice(&fixture[..fixture.len() - 1])
        });
        assert_eq!(
            conduit_browser_rendezvous_cbor_decode(
                fixture.len() - 1,
                (now >> 32) as u32,
                now as u32,
            ),
            ERROR_TRUNCATED
        );
    }
}
