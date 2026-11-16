//! Canonical planned placement identity, shared with bounded preparation.
//! This descriptive identity grants no reservation, possession or execution.
use sha2::{Digest, Sha256};

/// Preserve the planner's existing `placement:plot:gear:host:capability`
/// transcript and lowercase SHA-256 encoding without allocating scratch text.
/// Callers supply independently checked Source and selected Host/Back facts.
/// Boot and offer generation are checked separately by the exact Plan.
pub fn planned_placement_identity_bytes(
    checked_plot: &str,
    gear: &str,
    host: &str,
    capability: &str,
) -> [u8; 64] {
    let mut hash = Sha256::new();
    hash.update(b"placement:");
    for (index, field) in [checked_plot, gear, host, capability]
        .into_iter()
        .enumerate()
    {
        if index != 0 {
            hash.update(b":");
        }
        hash.update(field.as_bytes());
    }
    let digest = hash.finalize();
    let mut encoded = [0; 64];
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (pair, byte) in encoded.as_chunks_mut::<2>().0.iter_mut().zip(digest) {
        pair[0] = HEX[usize::from(byte >> 4)];
        pair[1] = HEX[usize::from(byte & 15)];
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn retains_existing_planner_identity() {
        assert_eq!(
            &planned_placement_identity_bytes(
                "checked/plot",
                "gear/sample",
                "host/green",
                "back/temperature"
            ),
            b"36b873f16935dbd9a633a8487e688a6ea1d0ec50a7a4c8d32b8e44cc0d8138e4",
        );
    }

    #[test]
    fn streamed_fields_match_previous_planner_transcript() {
        for fields in [
            ["", "", "", ""],
            ["checked:plot", "gear/sample", "host/blue", "back/native@1"],
            ["plot/温度", "gear/°", "host/green", "back/a:b"],
        ] {
            let old = Sha256::digest(
                format!(
                    "placement:{}:{}:{}:{}",
                    fields[0], fields[1], fields[2], fields[3]
                )
                .as_bytes(),
            );
            let mut text = alloc::string::String::new();
            for byte in old {
                use core::fmt::Write;
                write!(&mut text, "{byte:02x}").unwrap();
            }
            assert_eq!(
                planned_placement_identity_bytes(fields[0], fields[1], fields[2], fields[3])
                    .as_slice(),
                text.as_bytes()
            );
        }
    }
}
