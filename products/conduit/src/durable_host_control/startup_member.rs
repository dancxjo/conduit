//! Authenticated control of one complete, already prepared member Body kernel.
//! A lost carrier leaves this actor's exact ticket retained; it never releases it.
use super::*;
use conduit_body::{BodyBiographyEvidence, BodyPlan, BodyPlayIdentity, MembershipCredential, Wake};
use conduit_std_host::body_execution::{
    CommittedBodyStartup, DelegatedBodyPreparationReceipt, DelegatedBodyProgress,
    DelegatedBodyRequest, DelegatedBodyTerminalReceipt, PreparedDelegatedBody,
    StartupOwnerChallenge,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Session {
    pub(crate) id: [u8; 32],
    pub(crate) member: MembershipCredential,
    pub(crate) owner: conduit_body::FederationPeer,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
pub(crate) enum Command {
    BeginResume {
        session: Box<Session>,
        challenge: Box<StartupOwnerChallenge>,
    },
    Resume {
        session_id: [u8; 32],
        challenge: Box<StartupOwnerChallenge>,
        owner_signature: Signature,
    },
    Prepare {
        session_id: [u8; 32],
        wake: Box<Wake>,
        plan: Box<BodyPlan>,
        global_play: BodyPlayIdentity,
    },
    Commit {
        session_id: [u8; 32],
        preparation_id: String,
        biography: Box<BodyBiographyEvidence>,
    },
    Poll {
        session_id: [u8; 32],
        preparation_id: String,
    },
    Cancel {
        session_id: [u8; 32],
        preparation_id: String,
    },
    Release {
        session_id: [u8; 32],
        preparation_id: String,
    },
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub(crate) enum Progress {
    Prepared,
    Running,
    Terminal,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "outcome", deny_unknown_fields)]
pub(crate) enum Reply {
    Challenge {
        session_id: [u8; 32],
        challenge: Box<StartupOwnerChallenge>,
        member_signature: Signature,
    },
    Resumed {
        session_id: [u8; 32],
        member_offer_digest: [u8; 32],
    },
    Prepared {
        receipt: Box<DelegatedBodyPreparationReceipt>,
    },
    Committed {
        receipt: Box<DelegatedBodyPreparationReceipt>,
    },
    Progress {
        receipt: Box<DelegatedBodyPreparationReceipt>,
        progress: Progress,
        terminal: Option<Box<DelegatedBodyTerminalReceipt>>,
    },
    Released {
        session_id: [u8; 32],
        preparation_id: String,
    },
    Refused {
        code: String,
    },
}

enum Authentication {
    Pending {
        challenge: Box<StartupOwnerChallenge>,
        member_signature: [u8; 64],
    },
    Authenticated(Box<StartupOwnerChallenge>),
    Failed,
}

mod authentication;
pub(crate) mod preflight;
mod signature;
pub(crate) use signature::Signature;

pub(super) struct MemberRun {
    authentication: Authentication,
    session: Session,
    scope: Option<(Box<Wake>, Box<BodyPlan>, BodyPlayIdentity)>,
    prepared: Option<PreparedDelegatedBody>,
    control: conduit_std_host::RunControl,
    cancellation_request: Option<conduit_std_host::RunControlRequestId>,
    cancellation_play: [u8; 64],
    remaining_frames: u32,
}

impl DurableHostRuntime {
    pub(super) fn startup_member_custody_retained(&self) -> bool {
        self.startup_member
            .as_ref()
            .is_some_and(|run| run.prepared.is_some() || run.scope.is_some())
    }
    pub(super) fn discard_startup_handshake(&mut self) {
        if !self.startup_member_custody_retained() {
            self.startup_member.take();
        }
    }
    pub(super) fn startup_member_command(&mut self, command: Command) -> Result<Reply, String> {
        if let Command::BeginResume { session, challenge } = command {
            return self.begin_startup_member_resume(session, challenge);
        }
        if let Command::Resume {
            session_id,
            challenge,
            owner_signature,
        } = command
        {
            return self.complete_startup_member_resume(session_id, challenge, owner_signature);
        }
        let current = self
            .startup_member
            .as_mut()
            .ok_or("startup session is absent")?;
        let (session_id, preparation_id) = match &command {
            Command::Prepare { session_id, .. } => (*session_id, None),
            Command::Commit {
                session_id,
                preparation_id,
                ..
            }
            | Command::Poll {
                session_id,
                preparation_id,
            }
            | Command::Cancel {
                session_id,
                preparation_id,
            }
            | Command::Release {
                session_id,
                preparation_id,
            } => (*session_id, Some(preparation_id)),
            Command::BeginResume { .. } | Command::Resume { .. } => unreachable!(),
        };
        if !matches!(current.authentication, Authentication::Authenticated(_)) {
            return Err("startup original owner is not authenticated".into());
        }
        if session_id != current.session.id
            || preparation_id.is_some_and(|id| {
                current
                    .prepared
                    .as_ref()
                    .is_none_or(|p| &p.receipt().preparation_id != id)
            })
        {
            return Err("startup request does not own the retained preparation".into());
        }
        current.remaining_frames = current
            .remaining_frames
            .checked_sub(1)
            .ok_or("startup authenticated frame budget exhausted")?;
        match command {
            Command::Prepare {
                wake,
                plan,
                global_play,
                ..
            } => {
                if current.prepared.is_some() || current.scope.is_some() {
                    return Err("startup preparation is single attempt".into());
                }
                let Authentication::Authenticated(authenticated) = &current.authentication else {
                    return Err("startup authentication was retired".into());
                };
                if global_play != authenticated.global_play
                    || plan.plan_id != authenticated.plan_id
                    || wake.wake_id != authenticated.wake_id
                {
                    return Err("startup preparation differs from authenticated Body Play".into());
                }
                let actual = self.host.advertisement();
                if wake.body_id != current.session.member.body_id
                    || plan.body_id != current.session.member.body_id
                    || plan.plots.iter().any(|p| {
                        p.plan.fragments.iter().any(|f| {
                            f.host_id != actual.host_id
                                || f.boot_id != actual.boot_id
                                || f.offer_generation != actual.offer_generation
                        })
                    })
                {
                    return Err("startup Body scope does not match the exact member".into());
                }
                let root = self
                    .birth_root
                    .as_deref()
                    .ok_or("member service has no installed state")?;
                authentication::validate_current(&current.session, authenticated, actual, root)?;
                // A constructor refusal is known pre-effect, not an unknown ticket.
                let prepared = self.host.prepare_delegated_body(
                    DelegatedBodyRequest {
                        wake: &wake,
                        plan: &plan,
                        global_play: &global_play,
                        control: &current.control,
                    },
                    &mut conduit_std_host::ThreadTimer,
                )?;
                let receipt = prepared.receipt().clone();
                if receipt.member_play.active_play_id.as_str().len()
                    != current.cancellation_play.len()
                {
                    return Err("member kernel Play does not have canonical digest width".into());
                }
                current
                    .cancellation_play
                    .copy_from_slice(receipt.member_play.active_play_id.as_str().as_bytes());
                current.scope = Some((wake, plan, global_play));
                current.prepared = Some(prepared);
                Ok(Reply::Prepared {
                    receipt: Box::new(receipt),
                })
            }
            Command::Commit { biography, .. } => {
                let (wake, plan, play) = current.scope.as_ref().ok_or("startup scope is absent")?;
                if biography.body_id != current.session.member.body_id
                    || biography.membership.parts.iter().all(|part| {
                        part.current.as_ref().is_none_or(|host| {
                            host.host_id != current.session.owner.host_id
                                || host.boot_id != current.session.owner.boot_id
                                || host.offer_generation != current.session.owner.offer_generation
                        })
                    })
                    || plan.validate_for(wake).is_err()
                {
                    return Err("startup commit lost its original owner or Body scope".into());
                }
                let committed = CommittedBodyStartup::from_retained(&biography, plan, play)?;
                let prepared = current
                    .prepared
                    .as_mut()
                    .ok_or("startup preparation is absent")?;
                self.host.commit_delegated_startup(prepared, &committed)?;
                current.scope.take();
                Ok(Reply::Committed {
                    receipt: Box::new(prepared.receipt().clone()),
                })
            }
            Command::Poll { .. } | Command::Cancel { .. } => {
                let prepared = current
                    .prepared
                    .as_mut()
                    .ok_or("startup preparation is absent")?;
                if matches!(command, Command::Cancel { .. }) {
                    self.host.cancel_delegated_body(prepared)?;
                }
                let active_play = &current.cancellation_play;
                let request = std::cell::RefCell::new(current.cancellation_request.take());
                let signal = self.cancellation_signal.as_deref();
                let cancelled = || {
                    let observed =
                        signal.is_some_and(|path| cancellation_matches(path, active_play));
                    if observed {
                        if let Some(request) = request.borrow_mut().take() {
                            let _ = current.control.request_stop(request);
                        }
                    }
                    observed
                };
                let progress = self.host.poll_delegated_body_with_cancel(
                    prepared,
                    &mut std::io::sink(),
                    &mut conduit_std_host::ThreadTimer,
                    &current.control,
                    &cancelled,
                );
                current.cancellation_request = request.into_inner();
                let progress = progress?;
                let progress = match progress {
                    DelegatedBodyProgress::Prepared => Progress::Prepared,
                    DelegatedBodyProgress::Running => Progress::Running,
                    DelegatedBodyProgress::Terminal => Progress::Terminal,
                };
                Ok(Reply::Progress {
                    receipt: Box::new(prepared.receipt().clone()),
                    progress,
                    terminal: prepared.terminal_receipt().cloned().map(Box::new),
                })
            }
            Command::Release { preparation_id, .. } => {
                let prepared = current
                    .prepared
                    .as_mut()
                    .ok_or("startup preparation is absent")?;
                if prepared.terminal_receipt().is_none() {
                    return Err("startup release requires actual terminal evidence".into());
                }
                // Keep the ticket on any refusal; only positive release drops it.
                self.host.release_delegated_body(prepared)?;
                self.startup_member = None;
                Ok(Reply::Released {
                    session_id,
                    preparation_id,
                })
            }
            Command::BeginResume { .. } | Command::Resume { .. } => unreachable!(),
        }
    }
}

#[cfg(unix)]
pub(crate) fn call(state: &Path, command: Command) -> Result<Reply, String> {
    match super::body::call(
        state,
        Request::StartupMember {
            protocol: PROTOCOL,
            token: super::body::token(state)?,
            command: Box::new(command),
        },
    )? {
        Response::StartupMember { reply, .. } => Ok(*reply),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong startup member control response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn call(_: &Path, _: Command) -> Result<Reply, String> {
    Err("startup member requires local Unix control".into())
}

#[cfg(unix)]
pub(crate) fn owner_peer(state: &Path) -> Result<conduit_body::FederationPeer, String> {
    match super::body::call(
        state,
        Request::BodyMemberCarrierReceiver {
            protocol: PROTOCOL,
            token: super::body::token(state)?,
        },
    )? {
        Response::BodyMemberCarrierReceiver { peer, .. } => Ok(peer),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong original owner identity response".into()),
    }
}
#[cfg(unix)]
pub(crate) fn prepare_owner_identity(state: &Path) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(&owner_peer(state)?).map_err(|_| "owner peer encoding failed")?
    );
    Ok(())
}
#[cfg(not(unix))]
pub(crate) fn prepare_owner_identity(_: &Path) -> Result<(), String> {
    Err("original owner identity setup requires local Unix control".into())
}

fn cancellation_matches(path: &Path, active_play: &[u8]) -> bool {
    use std::io::Read;
    if active_play.len() > conduit_body::MAX_LIFECYCLE_ID_BYTES {
        return false;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut bytes = [0; conduit_body::MAX_LIFECYCLE_ID_BYTES + 1];
    let mut length = 0;
    while length < bytes.len() {
        match file.read(&mut bytes[length..]) {
            Ok(0) => break,
            Ok(count) => length += count,
            Err(_) => return false,
        }
    }
    &bytes[..length] == active_play
}

#[cfg(test)]
pub(crate) mod test_fixture;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use tests::fixture::{completed_custody, prepared_wire_custody, TerminalControl};
