//! Finite independent canonical Look transport on the admitted browser carrier.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;
fn correlation(value: &str) -> bool {
    value
        .strip_prefix("browser-history:")
        .is_some_and(|digits| {
            !digits.is_empty()
                && digits.len() <= 16
                && !digits.starts_with('0')
                && digits.bytes().all(|b| b.is_ascii_digit())
        })
}
pub(super) fn validate_request(
    id: &str,
    request: &OwnerFaceSnapshotRequest,
    show: &MaskShow,
    look: &[u8],
) -> Result<(), BrowserAdmissionFrameError> {
    if !correlation(id)
        || !request.has_exact_basis()
        || show.show.host_id != request.host_id
        || show.show.boot_id != request.boot_id
        || show.show.body_id.as_ref() != Some(&request.body_id)
        || look.is_empty()
        || look.len() > conduit_face::MAX_LOOK_BYTES
        || conduit_face::LookInfo::decode(look).is_err()
        || serde_json::to_vec(show)
            .map_err(|_| BrowserAdmissionFrameError::InvalidFaceSnapshot)?
            .len()
            > 64 * 1024
    {
        return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
    }
    Ok(())
}
pub(super) fn validate_response(
    id: &str,
    accepted: bool,
    refusal: &str,
) -> Result<(), BrowserAdmissionFrameError> {
    if !correlation(id) || refusal.len() > 128 || accepted != refusal.is_empty() {
        return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_response_correlation_and_outcomes_are_closed() {
        for (accepted, refusal) in [(true, ""), (false, "stale-face-or-show")] {
            let mut bytes = [0; 4096];
            let frame = BrowserAdmissionEgress::FaceHistoryMaskResponse {
                protocol: 1,
                request_id: "browser-history:1".into(),
                accepted,
                refusal: refusal.into(),
            };
            assert!(encode_browser_admission_frame(&frame, &mut bytes).is_ok());
        }
        for id in [
            "browser-component:1",
            "browser-history:0",
            "browser-history:01",
            "browser-history:1e2",
            "browser-history:10000000000000000",
        ] {
            assert!(validate_response(id, true, "").is_err());
        }
        assert!(validate_response("browser-history:1", true, "refused").is_err());
        assert!(validate_response("browser-history:1", false, "").is_err());
        assert!(validate_response("browser-history:1", false, &"x".repeat(129)).is_err());
    }
}
