use conduit_body::{body_play_digest, body_play_identity_matches};
use sha2::{Digest, Sha256};
#[test]
fn streaming_digest_preserves_the_existing_length_framed_body_play_identity() {
    for (body, wake, plan) in [
        ("body/1", "wake/1", "plan/1"),
        ("body/α", "wake/β", "plan/γ"),
    ] {
        for sequence in [0u64, 1, u64::MAX] {
            let mut legacy = Vec::new();
            for value in ["conduit.body/body-play@1", body, wake, plan] {
                legacy.extend_from_slice(&(value.len() as u32).to_le_bytes());
                legacy.extend_from_slice(value.as_bytes());
            }
            legacy.extend_from_slice(&sequence.to_le_bytes());
            let digest: [u8; 32] = Sha256::digest(&legacy).into();
            assert_eq!(body_play_digest(body, wake, plan, sequence), digest);
            let hex = digest
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let play = format!("body-play/sha256:{hex}");
            assert!(body_play_identity_matches(
                body, wake, plan, sequence, &play
            ));
            assert!(!body_play_identity_matches(
                body,
                wake,
                plan,
                sequence.wrapping_add(1),
                &play
            ));
            assert!(!body_play_identity_matches(
                body,
                wake,
                plan,
                sequence,
                &play.to_ascii_uppercase()
            ));
            assert!(!body_play_identity_matches(
                body, wake, plan, sequence, &hex
            ));
        }
    }
}
