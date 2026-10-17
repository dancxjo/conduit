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
    let (child, failure) = match error {
        ProtocolBodyRefusal::Call(crate::protocol_host_calls::ProtocolCallRefusal::Kernel(
            KernelCompositeError::Execution {
                child,
                reason: ChildExecutionError::Scheduler(SchedulerError::BackFailed(failure)),
            },
        )) => (Some(*child), *failure),
        ProtocolBodyRefusal::Call(error) => (None, error.failure()),
        _ => return Ok(None),
    };
    let mut text = crate::sign_format::FixedText::new();
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    if let ProtocolBodyRefusal::Call(
        crate::protocol_host_calls::ProtocolCallRefusal::UsbIsochronous(reason),
    ) = error
    {
        writeln!(text, "CONDUIT_PROTOCOL_USB_ISO_REFUSAL reason={reason:?}")?;
    }
    write!(
        text,
        "CONDUIT_PROTOCOL_STEP_FAILURE body={} plan={} play={} child=",
        play.body_id.as_str(),
        play.plan_id.as_str(),
        play.active_play_id.as_str()
    )?;
    match child {
        Some(child) => write!(text, "{child} origin=kernel")?,
        None => text.write_str("none origin=call_refusal")?,
    }
    writeln!(
        text,
        " code={} detail={}",
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
            body_id: serde_json::from_str("\"original-body\"").unwrap(),
            wake_id: serde_json::from_str("\"original-wake\"").unwrap(),
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
                    "CONDUIT_PROTOCOL_STEP_FAILURE body=original-body plan=body-plan/sha256:original-plan play=body-play/sha256:original-play child=3 origin=kernel code={} detail={}\n",
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

    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    #[test]
    fn native_dispatch_refusal_keeps_classifier_before_kernel_step() {
        let play = BodyPlayIdentity {
            active_play_id: "original-play".into(),
            body_id: serde_json::from_str("\"original-body\"").unwrap(),
            wake_id: serde_json::from_str("\"original-wake\"").unwrap(),
            plan_id: "original-plan".into(),
            play_sequence: 0,
        };
        for (native, expected) in [
            (
                crate::arch::usb_xhci::UsbError::StaleDeviceInstance,
                "code=invalid_lifecycle detail=1095",
            ),
            (
                crate::arch::usb_xhci::UsbError::DeviceVanished,
                "code=host_call_failed detail=1096",
            ),
        ] {
            let error = ProtocolBodyRefusal::Call(
                crate::protocol_host_calls::ProtocolCallRefusal::UsbProtocol(
                    crate::arch::UsbNativeProtocolRefusal::Host(
                        crate::arch::UsbNativeHostRefusal::Native(native),
                    ),
                ),
            );
            let report = format(&error, &play).unwrap().unwrap();
            let text = core::str::from_utf8(report.as_bytes()).unwrap();
            assert!(text.contains("child=none origin=call_refusal"));
            assert!(text.ends_with(&alloc::format!("{expected}\n")));
        }
    }
}
