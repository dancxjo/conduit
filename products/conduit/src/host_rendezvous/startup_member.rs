//! Existing secure joined-member carrier; it creates no membership or kernel.
use super::*;
use crate::durable_host_control::startup_member::{Command, Reply};
use conduit_std_host::body_execution::DelegatedBodyPreparationReceipt;

pub(super) fn serve(
    line: &mut impl RendezvousLine,
    state: &Path,
    first: Command,
) -> Result<(), String> {
    let actual_binding = line
        .startup_authentication_binding()
        .ok_or("startup requires the actual initialized authenticated TLS carrier")?;
    let Command::BeginResume { ref challenge, .. } = first else {
        return Err("startup carrier must begin with original owner authentication".into());
    };
    if challenge.transport_binding_sha256 != actual_binding {
        return Err("startup scope differs from the actual TLS provider".into());
    }
    let mut command = first;
    let mut prepared: Option<DelegatedBodyPreparationReceipt> = None;
    let mut session_id = None;
    loop {
        let release = matches!(&command, Command::Release { .. });
        let reply = if let (
            Command::Poll {
                session_id,
                preparation_id,
            },
            Some(receipt),
        ) = (&command, &prepared)
        {
            poll_interruptibly(line, state, *session_id, preparation_id, receipt)?
        } else {
            crate::durable_host_control::startup_member::call(state, command)?
        };
        match &reply {
            Reply::Resumed {
                session_id: actual, ..
            } => {
                session_id = Some(*actual);
                line.enter_retained_idle()?;
            }
            Reply::Prepared { receipt } => prepared = Some((**receipt).clone()),
            _ => (),
        }
        send(
            line,
            &Egress::StartupMember {
                protocol: PROTOCOL,
                reply: &reply,
            },
        )?;
        if release && matches!(reply, Reply::Released { .. }) {
            return line.close();
        }
        let bytes = line.receive()?;
        if bytes.len() > conduit_std_host::body_execution::STARTUP_MEMBER_CONTROL_FRAME_BYTES {
            return Err("startup joined frame exceeds its finite bound".into());
        }
        let next_frame: Ingress =
            serde_json::from_slice(&bytes).map_err(|_| "invalid startup joined frame")?;
        let Ingress::StartupMember {
            protocol,
            command: next,
        } = next_frame
        else {
            return Err("startup carrier changed its control protocol".into());
        };
        if protocol != PROTOCOL {
            return Err("startup carrier protocol changed".into());
        }
        if session_id.is_none() && !matches!(&*next, Command::Resume { .. }) {
            return Err("startup owner proof must precede preparation".into());
        }
        command = *next;
    }
}

fn poll_interruptibly(
    line: &mut impl RendezvousLine,
    state: &Path,
    session_id: [u8; 32],
    preparation_id: &str,
    receipt: &DelegatedBodyPreparationReceipt,
) -> Result<Reply, String> {
    if preparation_id != receipt.preparation_id || !line.supports_interruptible_exchange() {
        return Err("startup poll lost its actual prepared receiver".into());
    }
    // This is the existing rendezvous forwarding pattern: only the local
    // control round trip waits here; the sole kernel remains in its real actor.
    std::thread::scope(|scope| {
        let request = Command::Poll {
            session_id,
            preparation_id: preparation_id.into(),
        };
        let forwarding =
            scope.spawn(|| crate::durable_host_control::startup_member::call(state, request));
        let mut cancellation = None;
        while !forwarding.is_finished() {
            let Some(bytes) = line.poll_receive(Duration::from_millis(25))? else {
                continue;
            };
            if bytes.len() > conduit_std_host::body_execution::STARTUP_MEMBER_CONTROL_FRAME_BYTES {
                return Err("startup cancellation exceeds its finite frame".into());
            }
            if cancellation.is_some() {
                return Err("startup cancellation is already pending".into());
            }
            let ingress: Ingress =
                serde_json::from_slice(&bytes).map_err(|_| "invalid startup cancellation frame")?;
            let Ingress::StartupMember { protocol, command } = ingress else {
                return Err("startup poll received another protocol".into());
            };
            let Command::Cancel {
                session_id: candidate,
                preparation_id: id,
            } = *command
            else {
                return Err("startup poll accepts only its exact cancellation".into());
            };
            if protocol != PROTOCOL || candidate != session_id || id != preparation_id {
                return Err("startup cancellation belongs to another receiver".into());
            }
            crate::durable_host_control::signal_remote_cancellation(
                state,
                receipt.member_play.active_play_id.as_str(),
            )?;
            cancellation = Some(Command::Cancel {
                session_id,
                preparation_id: id,
            });
        }
        let reply = forwarding
            .join()
            .map_err(|_| "startup forwarding panicked")??;
        if let Some(cancel) = cancellation {
            // Send the original poll result first; its terminal facts cannot be
            // rewritten merely because cancellation raced an already completed cue.
            send(
                line,
                &Egress::StartupMember {
                    protocol: PROTOCOL,
                    reply: &reply,
                },
            )?;
            if matches!(
                &reply,
                Reply::Progress {
                    progress: crate::durable_host_control::startup_member::Progress::Terminal,
                    ..
                }
            ) {
                return Ok(reply);
            }
            return crate::durable_host_control::startup_member::call(state, cancel);
        }
        Ok(reply)
    })
}

#[cfg(test)]
pub(crate) fn wire_fixture_exchange(
    bytes: &[u8],
    dispatch: impl FnOnce(Command) -> Result<Reply, String>,
) -> Result<Vec<u8>, String> {
    let frame: Ingress = serde_json::from_slice(bytes).map_err(|_| "invalid startup wire frame")?;
    let Ingress::StartupMember { protocol, command } = frame else {
        return Err("wrong startup wire protocol".into());
    };
    if protocol != PROTOCOL {
        return Err("wrong startup wire version".into());
    }
    let reply = dispatch(*command)?;
    serde_json::to_vec(&Egress::StartupMember {
        protocol: PROTOCOL,
        reply: &reply,
    })
    .map_err(|_| "startup reply encoding failed".into())
}
