#![no_main]

use conduit_runtime::{EvidenceDecodeLimits, decode_event_ndjson_with_limits};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    if let Ok(input) = core::str::from_utf8(bytes) {
        let _ = decode_event_ndjson_with_limits(input, EvidenceDecodeLimits::default());
    }
});
