//! Wire exchange and terminal publication for exact admitted pool-member sessions.
//! Pool selection/execution remain owned by the Host and existing kernel.
use super::{encode_remote_frame, DurableRemoteExchange};
use conduit_core::PoolMemberSessionDirection;
use conduit_std_host::{
    hosted_local_model::LocalModelAdapterTerminal, pool_member_sessions::PoolMemberSessions,
    AdmittedLocalModelPoolMember, StdHost,
};
use conduit_wire::SessionMessage;

pub(super) fn exchange_admitted_pool_member_frame(
    host: &mut StdHost,
    member: &mut AdmittedLocalModelPoolMember,
    frame: conduit_wire::SessionFrame<'_>,
) -> Result<DurableRemoteExchange, String> {
    let message = frame.message;
    let direction = member
        .sessions()
        .iter()
        .find(|session| session.binding().identity() == frame.identity)
        .map(|session| session.direction)
        .ok_or_else(|| "pool-member-session-absent".to_string())?;
    {
        let session = member
            .sessions_mut()
            .session_for_binding_mut(&frame.identity)
            .ok_or_else(|| "pool-member-session-absent".to_string())?;
        session
            .machine_mut()
            .admit_inbound(frame)
            .map_err(|error| format!("admit pool member session frame: {error:?}"))?;
    }

    let mut responses = Vec::new();
    match message {
        SessionMessage::Hello(_) => {
            let session = member
                .sessions_mut()
                .session_for_binding_mut(&frame.identity)
                .ok_or_else(|| "pool-member-session-absent".to_string())?;
            responses.push(pool_member_response(session, SessionMessage::Ready)?);
        }
        SessionMessage::Offered { sequence, payload }
            if direction == PoolMemberSessionDirection::Input =>
        {
            {
                let session = member
                    .sessions_mut()
                    .session_for_binding_mut(&frame.identity)
                    .ok_or_else(|| "pool-member-input-session-absent".to_string())?;
                responses.push(pool_member_response(
                    session,
                    SessionMessage::Accepted { sequence },
                )?);
            }
            let mut output = Vec::new();
            let terminal = host.execute_local_model_pool_member(member, payload, &mut output);
            {
                let session = member
                    .sessions_mut()
                    .session_for_binding_mut(&frame.identity)
                    .ok_or_else(|| "pool-member-input-session-absent".to_string())?;
                responses.push(pool_member_response(
                    session,
                    SessionMessage::Delivered { sequence },
                )?);
            }
            complete_pool_member_operation(
                member.sessions_mut(),
                terminal,
                &output,
                &mut responses,
            )?;
        }
        SessionMessage::Offered { .. } => {
            return Err("pool-member-output-cannot-receive-offer".into());
        }
        SessionMessage::Cancelled { code } => {
            responses.extend(cancel_pool_member(member.sessions_mut(), code)?);
        }
        SessionMessage::Failed { code } => {
            responses.extend(fail_pool_member(member.sessions_mut(), code)?);
        }
        SessionMessage::Ready
        | SessionMessage::Pressure { .. }
        | SessionMessage::Accepted { .. }
        | SessionMessage::Delivered { .. }
        | SessionMessage::InputClosed { .. }
        | SessionMessage::InputAbnormal { .. }
        | SessionMessage::Terminal { .. } => {}
    }
    let active = member
        .sessions()
        .iter()
        .any(|session| session.machine().is_active());
    Ok(DurableRemoteExchange { responses, active })
}

fn pool_member_response(
    session: &mut conduit_std_host::pool_member_sessions::PoolMemberSession,
    message: SessionMessage<'_>,
) -> Result<Vec<u8>, String> {
    let binding = session.binding().clone();
    let response = binding.frame(message);
    session
        .machine_mut()
        .admit_outbound(response)
        .map_err(|error| format!("admit pool member session response: {error:?}"))?;
    encode_remote_frame(&binding, response)
}

fn fail_pool_member(sessions: &mut PoolMemberSessions, code: u16) -> Result<Vec<Vec<u8>>, String> {
    terminate_pool_member(
        sessions,
        SessionMessage::Failed { code },
        conduit_wire::SessionTerminalDisposition::Failed,
    )
}

fn cancel_pool_member(
    sessions: &mut PoolMemberSessions,
    code: u16,
) -> Result<Vec<Vec<u8>>, String> {
    terminate_pool_member(
        sessions,
        SessionMessage::Cancelled { code },
        conduit_wire::SessionTerminalDisposition::Cancelled,
    )
}

fn terminate_pool_member(
    sessions: &mut PoolMemberSessions,
    failure: SessionMessage<'_>,
    disposition: conduit_wire::SessionTerminalDisposition,
) -> Result<Vec<Vec<u8>>, String> {
    let mut responses = Vec::new();
    for session in sessions.iter_mut() {
        if !session.machine().is_active() {
            continue;
        }
        responses.push(pool_member_response(session, failure)?);
        responses.push(pool_member_response(
            session,
            SessionMessage::Terminal {
                disposition,
                final_sequence: session.machine().next_sequence(),
            },
        )?);
    }
    Ok(responses)
}

fn complete_pool_member_operation(
    sessions: &mut PoolMemberSessions,
    terminal: LocalModelAdapterTerminal,
    output: &[u8],
    responses: &mut Vec<Vec<u8>>,
) -> Result<(), String> {
    match terminal {
        LocalModelAdapterTerminal::Produced | LocalModelAdapterTerminal::Truncated => {
            let output_session = sessions
                .iter_mut()
                .find(|session| session.direction == PoolMemberSessionDirection::Output)
                .ok_or_else(|| "pool-member-output-session-absent".to_string())?;
            if output.len()
                > usize::try_from(output_session.binding().limits.maximum_payload_bytes)
                    .map_err(|_| "pool-member-output-limit-invalid".to_string())?
            {
                responses.extend(fail_pool_member(sessions, 73)?);
            } else {
                responses.push(pool_member_response(
                    output_session,
                    SessionMessage::Offered {
                        sequence: output_session.machine().next_sequence(),
                        payload: output,
                    },
                )?);
            }
        }
        LocalModelAdapterTerminal::Refused => {
            responses.extend(fail_pool_member(sessions, 70)?);
        }
        LocalModelAdapterTerminal::Failed | LocalModelAdapterTerminal::InvalidStructuredResult => {
            responses.extend(fail_pool_member(sessions, 71)?);
        }
        LocalModelAdapterTerminal::Cancelled => {
            responses.extend(cancel_pool_member(sessions, 72)?);
        }
        LocalModelAdapterTerminal::TimedOut => {
            responses.extend(fail_pool_member(sessions, 75)?);
        }
        LocalModelAdapterTerminal::ResponseBoundExceeded => {
            responses.extend(fail_pool_member(sessions, 76)?);
        }
        LocalModelAdapterTerminal::ProviderLost => {
            // Provider loss is terminal for this operation. The pool
            // kernel may select an alternative only for later work.
            responses.extend(fail_pool_member(sessions, 74)?);
        }
    }
    Ok(())
}

pub(super) fn encode_pool_member_hello_frames(
    sessions: &mut PoolMemberSessions,
) -> Result<Vec<Vec<u8>>, String> {
    let mut hello_frames = Vec::new();
    for session in sessions.iter_mut() {
        let binding = session.binding().clone();
        let hello = binding.hello_frame();
        session
            .machine_mut()
            .admit_outbound(hello)
            .map_err(|error| format!("admit pool member session grant: {error:?}"))?;
        hello_frames.push(encode_remote_frame(&binding, hello)?);
    }
    Ok(hello_frames)
}

#[cfg(test)]
#[path = "pool_member_frames/tests.rs"]
mod tests;
