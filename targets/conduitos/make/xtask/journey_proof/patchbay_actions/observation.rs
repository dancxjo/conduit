//! A terminal kernel refusal must not become an action observation timeout.
use super::{face_correspondence, ConduitosError, ACTION};

/// A new Face can precede its journey line on the same serial stream.
/// Observe the exact accepted request; never repeat the action while waiting.
pub(super) fn journey_observed(text: &str, request_id: &str) -> Result<bool, ConduitosError> {
    Ok(super::super::journey_records::decode(text)?
        .last()
        .is_some_and(|record| record["request_id"].as_str() == Some(request_id)))
}

pub(super) fn action_observed(text: &str, count: usize) -> Result<bool, ConduitosError> {
    for line in text
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        let Some(encoded) = line.strip_prefix("CONDUIT_KERNEL_SIGN ") else {
            continue;
        };
        let invalid = |detail| ConduitosError::refusal("patchbay-kernel-sign-invalid", detail);
        let sign: serde_json::Value = serde_json::from_str(encoded)
            .map_err(|error| invalid(format!("{error}: {}", line.trim_end())))?;
        if sign["schema"] != conduitos::sign_format::MACHINE_SIGN_SCHEMA {
            return Err(invalid(line.trim_end().to_owned()));
        }
        match sign["status"].as_str() {
            Some("accepted") => {}
            Some("refused") if sign["reason"].as_str().is_some_and(|s| !s.is_empty()) => {
                return Err(ConduitosError::refusal(
                    "patchbay-kernel-refused",
                    line.trim_end(),
                ));
            }
            _ => return Err(invalid(line.trim_end().to_owned())),
        }
    }
    Ok(face_correspondence::records(text, ACTION)?.len() > count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_handoff_waits_for_its_complete_journey_after_face() {
        let prior = "CONDUIT_PRODUCT_JOURNEY {\"request_id\":\"prior\"}\n";
        let face = format!("{prior}CONDUIT_NATIVE_FACE {{}}\n");
        assert!(!journey_observed(&face, "accepted").unwrap());
        let next = "CONDUIT_PRODUCT_JOURNEY {\"request_id\":\"accepted\"}";
        assert!(!journey_observed(&format!("{face}{next}"), "accepted").unwrap());
        let complete = format!("{face}{next}\n");
        assert!(journey_observed(&complete, "accepted").unwrap());
        assert!(!journey_observed(&complete, "foreign").unwrap());
        // Observation does not validate or manufacture editor state. The
        // caller's unchanged exact Source/Plan/Play assertions still own that.
        assert!(journey_observed(
            "CONDUIT_PRODUCT_JOURNEY {\"request_id\":\"accepted\",\"result\":\"wrong\"}\n",
            "accepted"
        )
        .unwrap());
        assert!(journey_observed("CONDUIT_PRODUCT_JOURNEY invalid\n", "accepted").is_err());
    }
    #[test]
    fn terminal_failure_precedes_stale_action_success_and_preserves_packet() {
        let packet = format!(
            "CONDUIT_KERNEL_SIGN {}\n",
            serde_json::json!({
                "schema": conduitos::sign_format::MACHINE_SIGN_SCHEMA,
                "status":"refused", "reason":"native-body-kernel-boundary-refused"
            })
        );
        let error = action_observed(&format!("{ACTION}{{}}\n{packet}"), 0)
            .unwrap_err()
            .to_string();
        assert!(error.contains("[patchbay-kernel-refused]"));
        assert!(error.contains(packet.trim_end()));
    }
    #[test]
    fn partial_and_nonterminal_records_do_not_fake_completion() {
        assert!(!action_observed("CONDUIT_KERNEL_SIGN {", 0).unwrap());
        assert!(!action_observed("CONDUIT_NATIVE_READING {}\n", 0).unwrap());
        assert!(action_observed(&format!("{ACTION}{{}}\n"), 0).unwrap());
        assert!(action_observed("CONDUIT_KERNEL_SIGN {}\n", 0).is_err());
    }
}
