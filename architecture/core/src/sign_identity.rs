//! Exact existing Sign transcript without retained canonical input bytes.
use super::*;
pub fn bind_sign(
    host_id: &HostId,
    boot_id: &BootId,
    active_play_id: Option<&ActivePlayId>,
    sequence: u64,
) -> SignIdentity {
    let mut canonical = canonical_bytes::CanonicalHash::new();
    push_string(&mut canonical, "sign");
    push_string(&mut canonical, host_id.as_str());
    push_string(&mut canonical, boot_id.as_str());
    push_string(
        &mut canonical,
        active_play_id.map_or("no-active-play", ActivePlayId::as_str),
    );
    push_u64(&mut canonical, sequence);
    SignIdentity {
        sign_id: SignId::from(hex_digest(&canonical.finish())),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        active_play_id: active_play_id.cloned(),
        sequence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn legacy_bind_sign(
        host_id: &HostId,
        boot_id: &BootId,
        active_play_id: Option<&ActivePlayId>,
        sequence: u64,
    ) -> SignIdentity {
        let mut canonical = Vec::new();
        push_string(&mut canonical, "sign");
        push_string(&mut canonical, host_id.as_str());
        push_string(&mut canonical, boot_id.as_str());
        push_string(
            &mut canonical,
            active_play_id.map_or("no-active-play", ActivePlayId::as_str),
        );
        push_u64(&mut canonical, sequence);
        SignIdentity {
            sign_id: SignId::from(hash_bytes(&canonical)),
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            active_play_id: active_play_id.cloned(),
            sequence,
        }
    }

    #[test]
    fn streamed_sign_matches_the_legacy_transcript() {
        for length in [1, 4096] {
            let host = HostId::from("h".repeat(length));
            let boot = BootId::from("b".repeat(length));
            for active in [None, Some(ActivePlayId::from("play/fixture"))] {
                for sequence in [0, u64::MAX] {
                    assert_eq!(
                        bind_sign(&host, &boot, active.as_ref(), sequence),
                        legacy_bind_sign(&host, &boot, active.as_ref(), sequence)
                    );
                }
            }
        }
    }
}
