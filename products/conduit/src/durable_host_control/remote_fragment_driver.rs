//! Bounded advancement and terminal session completion for admitted fragments.
use super::*;

pub(super) fn drive_remote_fragment<F>(
    host: &mut StdHost,
    admitted: &mut AdmittedRemoteFragment,
    pending_text: &mut Option<conduit_kernel::scheduler::HostCallRequest>,
    pending_native: &mut Option<conduit_kernel::scheduler::HostCallRequest>,
    responses: &mut Vec<Vec<u8>>,
    cancelled: F,
) -> Result<(), String>
where
    F: Fn() -> bool + Copy,
{
    if pending_text.is_some() || pending_native.is_some() {
        return Ok(());
    }
    const MAXIMUM_DRIVE_STEPS: usize = 64;
    for _ in 0..MAXIMUM_DRIVE_STEPS {
        if host.poll_remote_body_conversation_context(admitted)? {
            continue;
        }
        if let Some(request) = admitted.runtime_mut().next_host_request() {
            if admitted
                .runtime_mut()
                .complete_portable_host_call(request)?
            {
                continue;
            }
            if host.complete_remote_microphone_host_call(admitted, request, cancelled)? {
                continue;
            }
            if host.complete_remote_voice_host_call(admitted, request, cancelled)? {
                continue;
            }
            let work = admitted.runtime().describe_host_request(request)?;
            if work.contract_id.as_str() == "environment/read-environmental-source" {
                host.admitted_native_request(admitted, request)?;
                *pending_native = Some(request);
                return Ok(());
            }
            if work.contract_id.as_str() == conduit_core::PRESENT_HOST_CALL_CONTRACT {
                admitted.pending_worker_text_result(host.advertisement(), request)?;
                *pending_text = Some(request);
                return Ok(());
            }

            return Err(format!(
                "remote-host-call-unsupported:{}",
                work.contract_id.as_str()
            ));
        }
        let endpoints = admitted
            .sessions()
            .iter()
            .filter(|session| session.direction == RemoteCordDirection::Egress)
            .map(|session| session.endpoint)
            .collect::<Vec<_>>();
        for endpoint in endpoints {
            if let Some(transfer) = admitted.runtime_mut().next_egress(endpoint)? {
                responses.push(remote_response(
                    admitted,
                    endpoint,
                    SessionMessage::Offered {
                        sequence: transfer.sequence,
                        payload: &transfer.bytes,
                    },
                )?);
                return Ok(());
            }
        }
        match admitted.runtime_mut().step()? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => return Ok(()),
            SchedulerStatus::Drained => {
                complete_remote_sessions(admitted, responses)?;
                return Ok(());
            }
            SchedulerStatus::Cancelled => return Err("remote-fragment-cancelled".into()),
        }
    }
    Err("remote-fragment-drive-bound".into())
}

fn complete_remote_sessions(
    admitted: &mut AdmittedRemoteFragment,
    responses: &mut Vec<Vec<u8>>,
) -> Result<(), String> {
    let endpoints = admitted
        .sessions()
        .iter()
        .map(|session| (session.endpoint, session.direction))
        .collect::<Vec<_>>();
    let mut completions = Vec::with_capacity(endpoints.len());
    for (endpoint, direction) in endpoints {
        let checkpoint = admitted
            .sessions()
            .get(endpoint)
            .ok_or_else(|| "remote-session-absent".to_string())?
            .machine()
            .checkpoint();
        if direction == RemoteCordDirection::Egress {
            let disposition = admitted
                .runtime_mut()
                .egress_terminal_disposition(endpoint)?
                .ok_or_else(|| "remote-egress-not-terminal".to_string())?;
            completions.push((
                endpoint,
                direction,
                checkpoint.next_sequence,
                Some(disposition),
            ));
        } else if !checkpoint.input_closed {
            return Err("remote-ingress-not-closed".into());
        } else {
            completions.push((endpoint, direction, checkpoint.next_sequence, None));
        }
    }
    for (endpoint, direction, final_sequence, disposition) in completions {
        if direction == RemoteCordDirection::Egress {
            let message = match disposition.expect("egress completion has disposition") {
                conduit_kernel::RemoteTerminalDisposition::NormalClose => {
                    SessionMessage::InputClosed { final_sequence }
                }
                conduit_kernel::RemoteTerminalDisposition::Abnormal => {
                    return Err("typed-semantic-abnormal-egress-not-prepared".into());
                }
            };
            responses.push(remote_response(admitted, endpoint, message)?);
        }
        responses.push(remote_response(
            admitted,
            endpoint,
            SessionMessage::Terminal {
                disposition: conduit_wire::SessionTerminalDisposition::Completed,
                final_sequence,
            },
        )?);
    }
    Ok(())
}
