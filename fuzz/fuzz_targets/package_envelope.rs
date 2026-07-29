#![no_main]

use conduit_package::{PackageLimits, decode_package};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = decode_package(bytes, PackageLimits::default());
});
