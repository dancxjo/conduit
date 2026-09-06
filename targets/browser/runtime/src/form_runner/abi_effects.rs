//! Correlated completions preserve the session when stale callers are refused.
use super::*;

#[no_mangle]
pub extern "C" fn conduit_browser_form_pending_capacity() -> usize {
    crate::installed_browser::BROWSER_PENDING_REQUESTS
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_poll_effect() -> i32 {
    progress(|session| session.poll_effect())
}

/// Input is exact Play identity, placement identity, then optional canonical output.
#[no_mangle]
pub extern "C" fn conduit_browser_form_complete_effect(
    play_length: usize,
    placement_length: usize,
    request_sequence: u32,
    output_length: usize,
) -> i32 {
    complete_effect(
        play_length,
        placement_length,
        request_sequence,
        output_length,
        false,
    )
}
#[no_mangle]
pub extern "C" fn conduit_browser_form_acknowledge_cancellation(
    play_length: usize,
    placement_length: usize,
    request_sequence: u32,
) -> i32 {
    complete_effect(play_length, placement_length, request_sequence, 0, true)
}
fn complete_effect(
    play_length: usize,
    placement_length: usize,
    request_sequence: u32,
    output_length: usize,
    cancelled: bool,
) -> i32 {
    let Some(total) = play_length
        .checked_add(placement_length)
        .and_then(|n| n.checked_add(output_length))
    else {
        return ERROR_INPUT;
    };
    if play_length == 0 || placement_length == 0 || total > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = match (
            core::str::from_utf8(&input[..play_length]),
            core::str::from_utf8(&input[play_length..play_length + placement_length]),
        ) {
            (Ok(play), Ok(placement)) => progress(|session| {
                if cancelled {
                    return session.acknowledge_cancellation(play, placement, request_sequence);
                }
                session.complete_effect(
                    play,
                    placement,
                    request_sequence,
                    (output_length > 0).then_some(&input[play_length + placement_length..total]),
                )
            }),
            _ => ERROR_INPUT,
        };
        input[..total].fill(0);
        result
    })
}

fn progress(
    action: impl FnOnce(&mut TourSession) -> Result<super::super::TourProgress, String>,
) -> i32 {
    clear_output();
    SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(session) = slot.as_mut() else {
            return ERROR_NOT_RUNNING;
        };
        let result = match action(session) {
            Ok(result) => result,
            Err(message) => {
                #[derive(serde::Serialize)]
                struct CompletionRefusal<'a> {
                    schema: &'static str,
                    disposition: &'static str,
                    active_play_id: &'a str,
                    kernel_failure_code: Option<&'static str>,
                    kernel_failure_detail: Option<u16>,
                    message: &'a str,
                }
                let failure = session.scheduler.failure;
                let refusal = CompletionRefusal {
                    schema: "conduit.browser/completion-refusal@2",
                    disposition: if failure.is_some() {
                        "failed"
                    } else {
                        "refused"
                    },
                    active_play_id: session.active_play_id.as_str(),
                    kernel_failure_code: failure.map(|failure| failure.code.as_str()),
                    kernel_failure_detail: failure.map(|failure| failure.detail),
                    message: &message,
                };
                return if write_output(&refusal).is_ok() {
                    ERROR_COMPLETE
                } else {
                    ERROR_OUTPUT
                };
            }
        };
        if write_output(&result).is_err() {
            return ERROR_OUTPUT;
        }
        if matches!(result, super::super::TourProgress::Receipt(_)) {
            *slot = None;
        }
        STATUS_READY
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_correlated_completion_preserves_the_live_session() {
        let source = "form test {\n message: text/literal(\"SOS\")\n morse: text/morse(120)\n light: presentation/indicator\n message > morse > light\n}\n";
        let (session, _) = TourSession::prepare("browser/test", "boot/test", source, 1).unwrap();
        let play = session.active_play_id.as_str().to_owned();
        let request = session.pending[0].request;
        let placement = session.fragments[0].placements[usize::from(request.node.0)]
            .placement_id
            .as_str()
            .to_owned();
        SESSION.with(|slot| *slot.borrow_mut() = Some(session));
        assert_eq!(
            complete("stale", &placement, request.request.0),
            ERROR_COMPLETE
        );
        let refusal: serde_json::Value = OUTPUT.with(|output| {
            OUTPUT_LEN.with(|length| {
                serde_json::from_slice(&output.borrow()[..*length.borrow()]).unwrap()
            })
        });
        assert_eq!(refusal["schema"], "conduit.browser/completion-refusal@2");
        assert_eq!(refusal["disposition"], "refused");
        assert!(refusal["kernel_failure_detail"].is_null());
        assert!(refusal["kernel_failure_code"].is_null());
        assert_eq!(refusal["active_play_id"], play);
        SESSION.with(|slot| assert_eq!(slot.borrow().as_ref().unwrap().pending.len(), 1));
        assert_eq!(complete(&play, &placement, request.request.0), STATUS_READY);
        SESSION.with(|slot| assert!(slot.borrow().is_none()));
        assert_eq!(
            complete(&play, &placement, request.request.0),
            ERROR_NOT_RUNNING
        );
    }

    #[test]
    fn kernel_failure_category_and_detail_cross_the_completion_abi() {
        use conduit_kernel::{
            Failure, FailureCode, HostOperationDisposition, HostOperationOutcome,
        };
        let source = "form test {\n message: text/literal(\"SOS\")\n morse: text/morse(120)\n light: presentation/indicator\n message > morse > light\n}\n";
        for (code, expected) in [
            (FailureCode::HostOperationFailed, "host_operation_failed"),
            (FailureCode::StorageExhausted, "storage_exhausted"),
            (FailureCode::HostOperationDenied, "host_operation_denied"),
        ] {
            let (mut session, _) =
                TourSession::prepare("browser/test", "boot/test", source, 1).unwrap();
            let play = session.active_play_id.as_str().to_owned();
            let request = session.pending[0].request;
            // Fixture Host reports failure of the exact outstanding operation.
            // The ordinary scheduler and engine must retain its category.
            session
                .scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: if code == FailureCode::HostOperationDenied {
                            HostOperationDisposition::Denied
                        } else {
                            HostOperationDisposition::Failed
                        },
                        output: None,
                        failure: Some(Failure { code, detail: 42 }),
                    },
                )
                .unwrap();
            SESSION.with(|slot| *slot.borrow_mut() = Some(session));
            assert_eq!(conduit_browser_form_poll_effect(), ERROR_COMPLETE);
            let refusal: serde_json::Value = OUTPUT.with(|output| {
                OUTPUT_LEN.with(|length| {
                    serde_json::from_slice(&output.borrow()[..*length.borrow()]).unwrap()
                })
            });
            assert_eq!(refusal["schema"], "conduit.browser/completion-refusal@2");
            assert_eq!(refusal["disposition"], "failed");
            assert_eq!(refusal["kernel_failure_code"], expected);
            assert_eq!(refusal["kernel_failure_detail"], 42);
            assert_eq!(refusal["active_play_id"], play);
            SESSION.with(|slot| *slot.borrow_mut() = None);
        }
    }

    fn complete(play: &str, placement: &str, request: u32) -> i32 {
        INPUT.with(|input| {
            let mut input = input.borrow_mut();
            input[..play.len()].copy_from_slice(play.as_bytes());
            input[play.len()..play.len() + placement.len()].copy_from_slice(placement.as_bytes());
        });
        conduit_browser_form_complete_effect(play.len(), placement.len(), request, 0)
    }
}
