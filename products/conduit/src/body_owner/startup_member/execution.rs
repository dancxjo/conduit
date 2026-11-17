//! One original owner worker coordinates the single real prepared member kernel.
use super::carrier::Carrier;
use super::*;
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_std_host::{
    body_execution::DelegatedBodyPreparationReceipt, RunControl, RunControlRequestId, StdHost,
};
use std::{
    sync::{
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

struct Acknowledgment {
    _committed: conduit_std_host::body_execution::CommittedBodyStartup,
    frame_length: usize,
}

type Outcome = (
    StdHost,
    Box<Carrier>,
    Result<conduit_std_host::body_execution::DelegatedBodyTerminalReceipt, String>,
);
pub(in crate::durable_host::owner::controller) struct MemberWorker {
    control: RunControl,
    started: Receiver<(BodyPlayIdentity, Wake)>,
    acknowledge: Option<SyncSender<Result<Acknowledgment, String>>>,
    thread: Option<JoinHandle<Outcome>>,
    receipt: Option<Arc<DelegatedBodyPreparationReceipt>>,
    carrier: Option<Box<Carrier>>,
    unknown: bool,
    commit_failed: bool,
    workspace: Option<Arc<Mutex<conduit_std_host::body_execution::StartupMemberControlWorkspace>>>,
    session_id: [u8; 32],
}

impl Owner {
    pub(crate) fn start_member_service_run(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        selection: StartupMemberSelection,
    ) -> Result<super::super::continuing::RunWorker, String> {
        if !(1..=super::super::continuing::MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis)
            || self.host.is_playing()
            || self.session.realization().is_some()
        {
            return Err("member Body start requires one idle owner and finite deadline".into());
        }
        self.ensure_execution_retained(root)?;
        // Explicit setup is separate; this getter never mints or replaces a key.
        self.member_carrier_peer()?;
        let mut carrier = Carrier::connect(self, &selection)?;
        let sources = super::super::super::retained_source::workset(root)?
            .ok_or("retained complete workset Source is unavailable")?;
        let borrowed: Vec<_> = sources
            .iter()
            .map(|source| (&source.source, &source.plot))
            .collect();
        self.plan_startup_member_workset(
            &borrowed,
            &selection,
            &selection.member.host_advertisement,
            crate::durable_host::current_time_millis()?,
        )?;
        drop(borrowed);
        drop(sources);
        self.persist(root)?;
        super::super::state::require_execution_capacity(root)?;
        let proposal = self
            .session
            .realization()
            .ok_or("member Body proposal vanished")?
            .clone();
        let prospective = self
            .host
            .prospective_body_play(&proposal.wake, &proposal.plan)?;
        carrier.authenticate(
            self,
            &selection,
            &proposal.wake,
            &proposal.plan,
            &prospective,
            maximum_millis,
        )?;
        let receipt = match carrier.prepare(
            &proposal.wake,
            &proposal.plan,
            &prospective,
            &selection.member.host_advertisement,
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                // Never rewind the retained proposal or infer that the member
                // released a preparation whose response was lost.
                let next = self.session.clone();
                let unknown = serde_json::json!({
                    "schema":"conduit.body/member-execution@1", "disposition":"unknown",
                    "expected_global_play":prospective, "global_started":false,
                    "member_host":selection.member.host_advertisement.host_id,
                    "member_boot":selection.member.host_advertisement.boot_id,
                    "diagnostic":error,
                });
                self.retain_execution(root, next, unknown)?;
                return Err(
                    "member preparation outcome unknown; original proposal retained".into(),
                );
            }
        };
        let receipt = Arc::new(receipt);
        let worker_receipt = receipt.clone();
        let (workspace, session_id) = carrier.commit_workspace();
        let mut host = self.host.take_for_play()?;
        let control = RunControl::default();
        let worker_control = control.clone();
        let (started_tx, started) = mpsc::sync_channel(1);
        let (acknowledge, acknowledged) = mpsc::sync_channel::<Result<Acknowledgment, String>>(1);
        let thread = thread::Builder::new()
            .name("conduit-body-play".into())
            .spawn(move || {
                let outcome = (|| {
                    let (play, wake) = host.acknowledge_delegated_body_preparation(
                        &proposal.wake,
                        &proposal.plan,
                        &worker_receipt,
                    )?;
                    started_tx
                        .send((play, wake))
                        .map_err(|_| "original owner start observer disconnected")?;
                    let acknowledgment = acknowledged
                        .recv()
                        .map_err(|_| "original owner commit acknowledgment lost")??;
                    // Only the successfully retained owner biography crosses
                    // this channel. No bool or reconstructed Started is sent.
                    if worker_control.stop_requested() {
                        return carrier.cancel_prepared(&worker_receipt);
                    }
                    carrier.commit_encoded(&worker_receipt, acknowledgment.frame_length)?;
                    carrier.run(&worker_receipt, &worker_control, maximum_millis)
                })();
                (host, carrier, outcome)
            })
            .expect("member Body coordinator must start while owning the original Host");
        Ok(super::super::continuing::RunWorker::from_member(
            MemberWorker {
                control,
                started,
                acknowledge: Some(acknowledge),
                thread: Some(thread),
                receipt: Some(receipt),
                carrier: None,
                unknown: false,
                commit_failed: false,
                workspace: Some(workspace),
                session_id,
            },
        ))
    }
}

impl MemberWorker {
    pub(in crate::durable_host::owner::controller) fn progress(
        &mut self,
        owner: &mut Owner,
        root: &Path,
    ) -> Result<bool, String> {
        if self.unknown {
            return Ok(false);
        }
        match self.started.try_recv() {
            Ok((play, wake)) => {
                // Encode the exact projected transaction before durable Started.
                // This is owner retention preparation, never a kernel witness.
                let result = (|| {
                    let mut projected = owner.session.clone();
                    let authority = owner.host.advertisement();
                    projected
                        .started(
                            &authority.host_id,
                            &authority.boot_id,
                            play.clone(),
                            wake.clone(),
                        )
                        .map_err(debug)?;
                    let receipt = self.receipt.as_ref().ok_or("member receipt absent")?;
                    Carrier::encode_commit(
                        self.workspace
                            .as_ref()
                            .ok_or("member Commit workspace absent")?,
                        self.session_id,
                        receipt,
                        projected.evidence(),
                    )?;
                    drop(projected);
                    super::super::continuing::startup::retain_acknowledgment(
                        owner, root, play, wake,
                    )
                })()
                .and_then(|committed| {
                    committed.ok_or_else(|| {
                        "member startup omitted original causal evidence".to_string()
                    })
                })
                .and_then(|committed| {
                    let receipt = self.receipt.as_ref().ok_or("member receipt absent")?;
                    let frame_length = Carrier::encode_commit(
                        self.workspace
                            .as_ref()
                            .ok_or("member Commit workspace absent")?,
                        self.session_id,
                        receipt,
                        owner.session.evidence(),
                    )?;
                    Ok(Acknowledgment {
                        _committed: committed,
                        frame_length,
                    })
                });
                // The worker regains exclusive buffer access before network I/O.
                self.workspace.take();
                self.commit_failed = result.is_err();
                let acknowledge = self.acknowledge.take().ok_or("duplicate member Started")?;
                let _ = acknowledge.send(result);
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => (),
        }
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Ok(false);
        }
        let (host, mut carrier, outcome) = self
            .thread
            .take()
            .ok_or("member worker handle absent")?
            .join()
            .map_err(|_| "member coordinator panicked; outcome unknown")?;
        owner.host.restore_after_play(host)?;
        let preparation = self
            .receipt
            .as_ref()
            .ok_or("member preparation receipt absent")?;
        match outcome {
            Err(error) => {
                let receipt = serde_json::json!({
                    "schema":"conduit.body/member-execution@1", "disposition":"unknown",
                    "preparation":preparation.as_ref(), "diagnostic":error,
                    "owner_commit_failed":self.commit_failed,
                });
                self.carrier = Some(carrier);
                self.unknown = true;
                if self.commit_failed {
                    // A failed fsync may have committed the transaction. Do not
                    // overwrite it with an older biography or invent a Lull.
                    owner.last_execution = Some(receipt);
                    return Ok(false);
                }
                owner.retain_execution(root, owner.session.clone(), receipt)?;
                Ok(false)
            }
            Ok(terminal) => {
                let released = super::terminal_retention::finish(owner, root, &terminal, || {
                    carrier.release(preparation)
                });
                match released {
                    Ok(true) => (),
                    result => {
                        self.carrier = Some(carrier);
                        self.unknown = true;
                        return result;
                    }
                }
                // Receipt and decoded result owners drop before the workspace guard.
                drop(terminal);
                self.receipt.take();
                drop(carrier);
                Ok(true)
            }
        }
    }
    pub(in crate::durable_host::owner::controller) fn request_lull(&self) -> Result<(), String> {
        if self.unknown {
            return Err("member outcome requires explicit reconciliation".into());
        }
        self.control
            .request_stop(RunControlRequestId::new("owner/service-lull")?)
            .map_err(|_| "member Play already has a stop request".into())
    }
}
