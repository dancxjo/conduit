//! Preserve the original kernel failure beside the Root's terminal refusal.
use crate::protocol_source::ProtocolBodyRefusal;
use conduit_body::BodyPlayIdentity;
use conduit_composite::{ChildExecutionError, KernelCompositeError};
use conduit_kernel::scheduler::SchedulerError;
use core::fmt::Write;

pub(super) fn format(
    error: &ProtocolBodyRefusal,
    play: &BodyPlayIdentity,
) -> Result<Option<crate::sign_format::FixedText>, core::fmt::Error> {
    let ProtocolBodyRefusal::Call(crate::protocol_host_calls::ProtocolCallRefusal::Kernel(
        KernelCompositeError::Execution {
            child,
            reason: ChildExecutionError::Scheduler(SchedulerError::BackFailed(failure)),
        },
    )) = error
    else {
        return Ok(None);
    };
    let mut text = crate::sign_format::FixedText::new();
    writeln!(
        text,
        "CONDUIT_PROTOCOL_STEP_FAILURE body={} plan={} play={} child={} code={} detail={}",
        play.body_id.as_str(),
        play.plan_id.as_str(),
        play.active_play_id.as_str(),
        child,
        failure.code.as_str(),
        failure.detail
    )?;
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{Failure, FailureCode};

    #[test]
    fn original_stale_and_loss_failures_keep_distinct_codes_and_exact_play() {
        let play = BodyPlayIdentity {
            active_play_id: "body-play/sha256:original-play".into(),
            body_id: "original-body".into(),
            wake_id: "original-wake".into(),
            plan_id: "body-plan/sha256:original-plan".into(),
            play_sequence: 7,
        };
        for (code, detail) in [
            (FailureCode::InvalidLifecycle, 1095),
            (FailureCode::HostCallFailed, 1096),
        ] {
            let error =
                ProtocolBodyRefusal::Call(crate::protocol_host_calls::ProtocolCallRefusal::Kernel(
                    KernelCompositeError::Execution {
                        child: 3,
                        reason: ChildExecutionError::Scheduler(SchedulerError::BackFailed(
                            Failure { code, detail },
                        )),
                    },
                ));
            let report = format(&error, &play).unwrap().unwrap();
            assert_eq!(
                core::str::from_utf8(report.as_bytes()).unwrap(),
                alloc::format!(
                    "CONDUIT_PROTOCOL_STEP_FAILURE body=original-body plan=body-plan/sha256:original-plan play=body-play/sha256:original-play child=3 code={} detail={}\n",
                    code.as_str(),
                    detail
                )
            );
        }
        assert!(
            format(&ProtocolBodyRefusal::NotPlaying, &play)
                .unwrap()
                .is_none()
        );
    }
}
