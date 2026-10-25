//! A terminal kernel refusal must not become an action observation timeout.
use super::{ACTION, ConduitosError, face_correspondence};

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
