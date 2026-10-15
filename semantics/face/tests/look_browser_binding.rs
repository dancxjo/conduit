//! Browser uses the canonical Native wire, with its exact Source-owned profile.
use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;
use sha2::{Digest, Sha256};

#[test]
fn browser_profile_matches_checked_source_and_native_type_exactly() {
    let javascript = include_str!("../../../targets/browser/host/assets/application-look.mjs");
    let hex = javascript
        .split("const LOOK_TYPE_HEX = \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let bytes: Vec<_> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(
        bytes,
        LookInfo::semantic_type()
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    let source = [
        include_str!("../types.conduit"),
        include_str!("../look.conduit"),
    ]
    .join("\n");
    let digest = Sha256::digest(source.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert!(javascript.contains(&format!("LOOK_CONTRACT_SOURCE_SHA256 = \"{digest}\"")));
    let value = phosphor_look().unwrap();
    let golden = include_str!("../../../proof/conformance/face-look/phosphor-look-v1.hex")
        .split_whitespace()
        .collect::<String>();
    let golden: Vec<_> = golden
        .as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(core::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(value.clone().encode().unwrap(), golden);

    assert_eq!(
        LookInfo::decode(&value.clone().encode().unwrap()).unwrap(),
        value
    );
    if let Ok(path) = std::env::var("CONDUIT_LOOK_WIRE_FIXTURE") {
        std::fs::write(path, value.encode().unwrap()).unwrap();
    }
}
