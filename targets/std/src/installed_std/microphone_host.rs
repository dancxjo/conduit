//! Shared exact microphone invocation for local and remote admitted Host Calls.
use crate::hosted_microphone::{AlsaMicrophoneAdapter, MicrophoneFailure};
use conduit_kernel::{Failure, FailureCode, HostCallDisposition, HostCallOutcome};

pub(super) fn execute(
    input: &[u8],
    implementation_id: &str,
    adapter: Option<&mut AlsaMicrophoneAdapter>,
    cancelled: impl FnMut() -> bool,
) -> Result<Result<Vec<u8>, MicrophoneFailure>, String> {
    if input != b"capture" {
        return Err("microphone clip capture request is not exact".into());
    }
    let adapter =
        adapter.ok_or_else(|| "microphone clip request has no initialized adapter".to_string())?;
    if adapter.implementation_id() != implementation_id {
        return Err("microphone implementation differs from the exact planned Back".into());
    }
    Ok(adapter.capture_clip(cancelled))
}

pub(super) fn failure_outcome(error: MicrophoneFailure) -> HostCallOutcome {
    if error == MicrophoneFailure::Cancelled {
        HostCallOutcome {
            disposition: HostCallDisposition::Cancelled,
            output: None,
            failure: None,
        }
    } else {
        HostCallOutcome {
            disposition: HostCallDisposition::Failed,
            output: None,
            failure: Some(Failure {
                code: FailureCode::HostCallFailed,
                detail: error as u16,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_failure_and_cancellation_remain_distinct() {
        let cancelled = failure_outcome(MicrophoneFailure::Cancelled);
        assert_eq!(cancelled.disposition, HostCallDisposition::Cancelled);
        assert!(cancelled.failure.is_none());
        assert!(cancelled.output.is_none());
        let failed = failure_outcome(MicrophoneFailure::ShortCapture);
        assert_eq!(failed.disposition, HostCallDisposition::Failed);
        assert_eq!(
            failed.failure.unwrap(),
            Failure {
                code: FailureCode::HostCallFailed,
                detail: MicrophoneFailure::ShortCapture as u16,
            }
        );
        assert!(failed.output.is_none());
    }
}
