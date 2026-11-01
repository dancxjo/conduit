//! Private stable identity for one exact Source/check pair in the Birth inventory.
use alloc::string::String;
use conduit_body::{MAX_BODY_PLOT_IDENTITY_BYTES, ResidentPlot};
use sha2::{Digest, Sha256};

pub(crate) fn plot_choice_identity(plot: &ResidentPlot) -> String {
    // BirthDraft admits every pair through BodyWorkset before projection.
    // Its 2048-byte combined identity bound makes all framed lengths fit u32.
    const _: () = assert!(MAX_BODY_PLOT_IDENTITY_BYTES <= u32::MAX as usize);
    let mut digest = Sha256::new();
    for value in [
        "conduit.birth/plot-choice@1",
        plot.source_document_id.as_str(),
        plot.checked_plot_id.as_str(),
    ] {
        let length = u32::try_from(value.len()).expect("admitted bounded Birth identity");
        digest.update(length.to_le_bytes());
        digest.update(value.as_bytes());
    }
    // URL-safe unpadded base64 retains all 256 digest bits in 43 bytes.
    // Projection repeats subject identities in bounded wire records.
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = digest.finalize();
    let mut identity = String::with_capacity(43);
    let mut bits = 0u32;
    let mut available = 0u8;
    for byte in bytes {
        bits = (bits << 8) | u32::from(byte);
        available += 8;
        while available >= 6 {
            available -= 6;
            identity.push(ALPHABET[((bits >> available) & 63) as usize] as char);
        }
    }
    if available != 0 {
        identity.push(ALPHABET[((bits << (6 - available)) & 63) as usize] as char);
    }
    identity
}
