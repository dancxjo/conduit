#![no_main]

use conduit_embedded::{HilEventFrame, HilRequest, HilRunHeader};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    if let Ok(request) = <&[u8; HilRequest::ENCODED_BYTES]>::try_from(bytes) {
        let _ = HilRequest::decode(request);
    }
    if let Ok(header) = <&[u8; HilRunHeader::ENCODED_BYTES]>::try_from(bytes) {
        let _ = HilRunHeader::decode(header);
    }
    if let Ok(event) = <&[u8; HilEventFrame::ENCODED_BYTES]>::try_from(bytes) {
        let _ = HilEventFrame::decode(event);
    }
});
