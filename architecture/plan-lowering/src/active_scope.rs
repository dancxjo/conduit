//! Check the existing canonical active-Play digest without cloning owned IDs.
pub(crate) fn matches(active: &conduit_core::ActivePlayIdentity) -> bool {
    let bytes = active.active_play_id.as_str().as_bytes();
    if bytes.len() != 64 {
        return false;
    }
    let digest = conduit_core::active_play_digest(
        active.plan_id.as_str(),
        active.host_id.as_str(),
        active.boot_id.as_str(),
        active.play_sequence,
    );
    const HEX: &[u8; 16] = b"0123456789abcdef";
    digest
        .iter()
        .zip(bytes.as_chunks::<2>().0)
        .all(|(byte, pair)| {
            pair[0] == HEX[usize::from(byte >> 4)] && pair[1] == HEX[usize::from(byte & 15)]
        })
}

#[cfg(test)]
mod tests {
    use super::matches;
    use conduit_core::*;
    #[test]
    fn canonical_reference_and_every_scope_field_remain_exact() {
        for sequence in [0, 1, 42, u64::MAX] {
            let active = bind_active_play(
                &"plan/α".into(),
                &"host/1".into(),
                &"boot/1".into(),
                sequence,
            );
            assert!(matches(&active));
            for change in 0..7 {
                let mut other = active.clone();
                match change {
                    0 => other.plan_id = "plan/β".into(),
                    1 => other.host_id = "host/2".into(),
                    2 => other.boot_id = "boot/2".into(),
                    3 => other.play_sequence = sequence.wrapping_add(1),
                    4 => {
                        other.active_play_id =
                            active.active_play_id.as_str().to_ascii_uppercase().into()
                    }
                    5 => other.active_play_id = "00".into(),
                    _ => other.active_play_id = "g".repeat(64).into(),
                }
                assert!(!matches(&other));
            }
        }
    }
}
