//! One explicitly selected authenticated carrier and its original counter workspace.
use super::*;
use crate::durable_host_control::startup_member::{Command, Progress, Reply, Session, Signature};
use conduit_body::{BodyBiographyEvidence, BodyPlan, BodyPlayIdentity, FederationChallenge, Wake};
use conduit_std_host::{
    body_execution::{
        DelegatedBodyPreparationReceipt, DelegatedBodyTerminalReceipt,
        StartupMemberControlWorkspace, StartupOwnerChallenge,
    },
    secure_websocket::SecureWebSocketClientLine,
    RunControl, RunControlRequestId,
};
use serde::Serialize;
use sha2::Digest;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[path = "carrier/codec.rs"]
mod codec;

pub(super) struct Carrier {
    line: SecureWebSocketClientLine,
    session_id: [u8; 32],
    offer_digest: [u8; 32],
    transport_binding: [u8; 32],
    deadline_request: Option<RunControlRequestId>,
    timeout: Duration,
    workspace: Arc<Mutex<StartupMemberControlWorkspace>>,
}
enum ReceiveError {
    Waiting,
    Unknown,
    Malformed,
}

#[derive(Serialize)]
#[serde(tag = "operation")]
enum BorrowedCommand<'a> {
    Prepare {
        session_id: [u8; 32],
        wake: &'a Wake,
        plan: &'a BodyPlan,
        global_play: &'a BodyPlayIdentity,
    },
    Commit {
        session_id: [u8; 32],
        preparation_id: &'a str,
        biography: &'a BodyBiographyEvidence,
    },
    Poll {
        session_id: [u8; 32],
        preparation_id: &'a str,
    },
    Cancel {
        session_id: [u8; 32],
        preparation_id: &'a str,
    },
    Release {
        session_id: [u8; 32],
        preparation_id: &'a str,
    },
}

impl Carrier {
    pub(super) fn connect(
        owner: &Owner,
        selection: &StartupMemberSelection,
    ) -> Result<Box<Self>, String> {
        owner.require_current_native_member(&selection.member)?;
        selection.validate(
            &selection.member.host_advertisement,
            crate::durable_host::current_time_millis()?,
        )?;
        // This reserves new buffers before creating or sending on a carrier.
        let workspace = owner
            .host
            .prepare_startup_member_control_workspace()?
            .into_shared();
        let candidate = selection
            .rendezvous
            .candidates
            .iter()
            .find(|c| c.candidate_id == selection.candidate_id)
            .ok_or("selected startup candidate is absent")?;
        let address = crate::durable_host::routed_join::route_address(&candidate.reachability)?;
        let line = SecureWebSocketClientLine::connect_pinned(
            address,
            &candidate.reachability,
            &candidate.authentication.server_identity,
            candidate.authentication.transport_binding_sha256,
            Duration::from_millis(u64::from(candidate.attempt_timeout_millis)),
            conduit_std_host::body_execution::STARTUP_MEMBER_CONTROL_FRAME_BYTES as u32,
        )
        .map_err(|_| "startup member authenticated connection failed")?;
        let mut session_id = [0; 32];
        getrandom::fill(&mut session_id).map_err(|_| "startup session randomness unavailable")?;
        if session_id == [0; 32] {
            return Err("startup session nonce is invalid".into());
        }
        // Existing TLS/caller preparation owns this one carrier allocation.
        // Keep the same Box through the worker and all unknown outcomes.
        let mut carrier = Box::new(Self {
            line,
            session_id,
            offer_digest: [0; 32],
            transport_binding: candidate.authentication.transport_binding_sha256,
            deadline_request: Some(RunControlRequestId::new("owner/member-deadline")?),
            timeout: Duration::from_millis(u64::from(candidate.attempt_timeout_millis)),
            workspace,
        });
        carrier.offer_digest = carrier.hello(
            &mut selection.rendezvous.copy_session_secret_for_attempt(),
            &selection.member.host_advertisement,
        )?;
        Ok(carrier)
    }

    pub(super) fn authenticate(
        &mut self,
        owner: &Owner,
        selection: &StartupMemberSelection,
        wake: &Wake,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
        maximum_millis: u64,
    ) -> Result<(), String> {
        let now = crate::durable_host::current_time_millis()?;
        let session = Session {
            id: self.session_id,
            member: selection.member.credential.clone(),
            owner: owner.member_carrier_peer()?,
        };
        let challenge = StartupOwnerChallenge {
            body_id: plan.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            plan_id: plan.plan_id.clone(),
            global_play: play.clone(),
            part_id: session.member.part_id.clone(),
            credential_id: session.member.credential_id.clone(),
            member_offer_digest: self.offer_digest,
            transport_binding_sha256: self.transport_binding,
            federation: FederationChallenge {
                session_id: "pending".into(),
                line_id: "pending".into(),
                line_security: conduit_core::LineSecurity::AuthenticatedEncrypted,
                initiator: owner.startup_member_peer(selection)?,
                responder: session.owner.clone(),
                nonce: [1; 32],
                expires_at_millis: now
                    .checked_add(30_000)
                    .ok_or("startup auth expiry overflow")?,
                maximum_frames: u32::try_from(maximum_millis.div_ceil(25) + 16)
                    .map_err(|_| "startup frame budget overflow")?,
            },
        };
        self.send_command(&Command::BeginResume {
            session: Box::new(session),
            challenge: Box::new(challenge),
        })?;
        let Reply::Challenge {
            session_id,
            challenge,
            member_signature,
        } = self.receive()?
        else {
            return Err("startup member did not issue its original challenge".into());
        };
        if session_id != self.session_id
            || challenge.member_offer_digest != self.offer_digest
            || challenge.transport_binding_sha256 != self.transport_binding
            || challenge.federation.maximum_frames
                != u32::try_from(maximum_millis.div_ceil(25) + 16)
                    .map_err(|_| "startup frame budget overflow")?
        {
            return Err("startup challenge changed the actual selected carrier".into());
        }
        let owner_signature = owner.sign_startup_member_resume(
            selection,
            &challenge,
            crate::durable_host::current_time_millis()?,
        )?;
        conduit_body::FederatedReceivingSession::establish(
            challenge.federation.clone(),
            member_signature.0,
            owner_signature,
            crate::durable_host::current_time_millis()?,
        )
        .map_err(|_| "startup member original key proof failed")?;
        self.send_command(&Command::Resume {
            session_id,
            challenge,
            owner_signature: Signature(owner_signature),
        })?;
        match self.receive()? {
            Reply::Resumed {
                session_id,
                member_offer_digest,
            } if session_id == self.session_id && member_offer_digest == self.offer_digest => {
                Ok(())
            }
            _ => Err("startup original owner resume was refused".into()),
        }
    }

    pub(super) fn prepare(
        &mut self,
        wake: &Wake,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
        actual: &HostAdvertisement,
    ) -> Result<DelegatedBodyPreparationReceipt, String> {
        self.send_command(&BorrowedCommand::Prepare {
            session_id: self.session_id,
            wake,
            plan,
            global_play: play,
        })?;
        match self.receive()? {
            Reply::Prepared { receipt }
                if receipt.global_play == *play
                    && receipt.plan_id == plan.plan_id
                    && receipt.host_id == actual.host_id
                    && receipt.boot_id == actual.boot_id
                    && receipt.offer_generation == actual.offer_generation =>
            {
                Ok(*receipt)
            }
            _ => Err("startup preparation lost the original Body/member correlation".into()),
        }
    }

    pub(super) fn commit_workspace(&self) -> (Arc<Mutex<StartupMemberControlWorkspace>>, [u8; 32]) {
        (self.workspace.clone(), self.session_id)
    }
    pub(super) fn encode_commit(
        workspace: &Arc<Mutex<StartupMemberControlWorkspace>>,
        session_id: [u8; 32],
        receipt: &DelegatedBodyPreparationReceipt,
        biography: &BodyBiographyEvidence,
    ) -> Result<usize, String> {
        codec::encode_commit(workspace, session_id, receipt, biography)
    }
    pub(super) fn commit_encoded(
        &mut self,
        receipt: &DelegatedBodyPreparationReceipt,
        length: usize,
    ) -> Result<(), String> {
        self.send_encoded(length)?;
        match self.receive()? {
            Reply::Committed { receipt: actual } if *actual == *receipt => Ok(()),
            _ => Err("startup committed result differs from prepared member".into()),
        }
    }

    pub(super) fn cancel_prepared(
        &mut self,
        receipt: &DelegatedBodyPreparationReceipt,
    ) -> Result<DelegatedBodyTerminalReceipt, String> {
        self.send_command(&BorrowedCommand::Cancel {
            session_id: self.session_id,
            preparation_id: &receipt.preparation_id,
        })?;
        match self.receive()? {
            Reply::Progress {
                receipt: actual,
                progress: Progress::Terminal,
                terminal: Some(terminal),
            } if *actual == *receipt && terminal_matches(&terminal, receipt) => Ok(*terminal),
            _ => Err("startup prepared cancellation outcome unknown".into()),
        }
    }

    pub(super) fn run(
        &mut self,
        receipt: &DelegatedBodyPreparationReceipt,
        control: &RunControl,
        maximum_millis: u64,
    ) -> Result<DelegatedBodyTerminalReceipt, String> {
        let deadline = Instant::now() + Duration::from_millis(maximum_millis);
        let response_deadline = deadline + self.timeout;
        let limit = maximum_millis.div_ceil(25) + 8;
        let mut cancelled = false;
        for _ in 0..limit {
            self.send_command(&BorrowedCommand::Poll {
                session_id: self.session_id,
                preparation_id: &receipt.preparation_id,
            })?;
            self.line
                .set_read_timeout(Some(Duration::from_millis(25)))
                .map_err(|_| "startup carrier timeout unavailable")?;
            let mut replies = 1;
            while replies > 0 {
                if Instant::now() >= response_deadline {
                    return Err("startup cancellation/terminal outcome unknown".into());
                }
                if Instant::now() >= deadline && !control.stop_requested() {
                    if let Some(request) = self.deadline_request.take() {
                        let _ = control.request_stop(request);
                    }
                }
                if control.stop_requested() && !cancelled {
                    self.send_command(&BorrowedCommand::Cancel {
                        session_id: self.session_id,
                        preparation_id: &receipt.preparation_id,
                    })?;
                    cancelled = true;
                    replies += 1;
                }
                let reply = match self.receive_reply() {
                    Ok(reply) => reply,
                    Err(ReceiveError::Waiting) => continue,
                    Err(_) => return Err("startup member outcome unknown".into()),
                };
                replies -= 1;
                match reply {
                    Reply::Progress {
                        receipt: actual,
                        progress,
                        terminal,
                    } if *actual == *receipt => {
                        if let Some(terminal) = terminal {
                            if !terminal_matches(&terminal, receipt) {
                                return Err(
                                    "startup terminal belongs to another preparation".into()
                                );
                            }
                            // Drain the corresponding cancellation reply before reuse.
                            if replies > 0 {
                                match self.receive()? {
                                    Reply::Progress {
                                        receipt: echoed,
                                        terminal: Some(observed),
                                        ..
                                    } if *echoed == *receipt
                                        && terminal_matches(&observed, receipt) => {}
                                    _ => {
                                        return Err(
                                            "startup cancellation terminal reply differs".into()
                                        )
                                    }
                                }
                            }
                            return Ok(*terminal);
                        }
                        if matches!(progress, Progress::Terminal) {
                            return Err("startup terminal evidence is absent".into());
                        }
                    }
                    _ => return Err("startup member progress was refused or redirected".into()),
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err("startup member transport budget exhausted; outcome unknown".into())
    }

    pub(super) fn release(
        &mut self,
        receipt: &DelegatedBodyPreparationReceipt,
    ) -> Result<(), String> {
        self.send_command(&BorrowedCommand::Release {
            session_id: self.session_id,
            preparation_id: &receipt.preparation_id,
        })?;
        match self.receive()? {
            Reply::Released {
                session_id,
                preparation_id,
            } if session_id == self.session_id && preparation_id == receipt.preparation_id => {
                Ok(())
            }
            _ => Err("startup member release outcome unknown".into()),
        }
    }
    fn receive(&mut self) -> Result<Reply, String> {
        self.line
            .set_read_timeout(Some(self.timeout))
            .map_err(|_| "startup response timeout unavailable")?;
        self.receive_reply()
            .map_err(|_| "startup control reply outcome unknown".into())
    }
}
fn terminal_matches(t: &DelegatedBodyTerminalReceipt, p: &DelegatedBodyPreparationReceipt) -> bool {
    t.preparation_id == p.preparation_id
        && t.global_play == p.global_play
        && t.member_play == p.member_play
        && t.host_id == p.host_id
        && t.boot_id == p.boot_id
        && t.offer_generation == p.offer_generation
        && t.plan_id == p.plan_id
        && t.failure.as_ref().is_none_or(|s| s.len() <= 512)
        && t.cleanup_failure.as_ref().is_none_or(|s| s.len() <= 512)
        && t.startup_chimes.len()
            <= conduit_std_host::body_execution::STARTUP_MEMBER_MAXIMUM_CHIME_RECEIPTS
}
