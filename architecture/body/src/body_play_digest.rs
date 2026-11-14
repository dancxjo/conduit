//! The existing Body Play identity law without a temporary byte document.
use sha2::{Digest, Sha256};

pub fn body_play_digest(body: &str, wake: &str, plan: &str, sequence: u64) -> [u8; 32] {
    let mut hash = Sha256::new();
    for value in ["conduit.body/body-play@1", body, wake, plan] {
        hash.update((value.len() as u32).to_le_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(sequence.to_le_bytes());
    hash.finalize().into()
}

pub fn body_play_identity_matches(
    body: &str,
    wake: &str,
    plan: &str,
    sequence: u64,
    play: &str,
) -> bool {
    let Some(hex) = play.strip_prefix("body-play/sha256:") else {
        return false;
    };
    if hex.len() != 64 {
        return false;
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    body_play_digest(body, wake, plan, sequence)
        .iter()
        .zip(hex.as_bytes().as_chunks::<2>().0)
        .all(|(byte, pair)| {
            pair[0] == HEX[usize::from(byte >> 4)] && pair[1] == HEX[usize::from(byte & 15)]
        })
}
