//! Finite post-admission presence owned by the live native Body session.

use conduit_body::{BodyMembership, HostPresenceState, HostPresenceTable, MembershipCredential};
use conduit_core::{LinkBindingId, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionSocket,
    BrowserAdmissionSocketError, BrowserWebRtcRendezvous, BROWSER_ADMISSION_PROTOCOL,
};
use conduit_std_host::websocket::NativeWebSocketError;
use std::io::ErrorKind;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

const LEASE_MILLIS: u64 = 120_000;
const RENEW_AFTER_MILLIS: u64 = 30_000;

#[path = "browser_presence_return.rs"]
mod return_session;
#[path = "browser_presence/signaling.rs"]
mod signaling;

pub(super) struct BrowserPresenceCoordinator {
    clock: Instant,
    table: HostPresenceTable,
    workers: Vec<PresenceWorker>,
    credentials: Vec<MembershipCredential>,
    rendezvous: BrowserWebRtcRendezvous,
    session_sequence: u64,
    sign_sequence: u64,
}

struct PresenceWorker {
    credential: MembershipCredential,
    session_id: LinkBindingId,
    receiver: Receiver<WorkerEvent>,
    outbound: SyncSender<BrowserAdmissionEgress>,
}

enum WorkerEvent {
    Renewal {
        frame: Box<BrowserAdmissionIngress>,
        response: SyncSender<WorkerResponse>,
    },
    Lost,
    Failed(String),
}

enum WorkerResponse {
    Accepted {
        sequence: u64,
        expires_at_millis: u64,
    },
    Refused(String),
    Relayed,
    WebRtcGrant {
        index: u16,
        total: u16,
        grant: Option<conduit_std_host::browser_admission::BrowserWebRtcGrant>,
    },
}

impl BrowserPresenceCoordinator {
    pub(super) fn new(body_id: conduit_body::BodyId) -> Self {
        Self {
            clock: Instant::now(),
            table: HostPresenceTable::new(body_id, LEASE_MILLIS)
                .expect("fixed browser presence lease is valid"),
            workers: Vec::with_capacity(conduit_body::MAX_BODY_PARTS),
            credentials: Vec::with_capacity(conduit_body::MAX_BODY_PARTS),
            rendezvous: BrowserWebRtcRendezvous::default(),
            session_sequence: 0,
            sign_sequence: 0,
        }
    }

    pub(super) fn table(&self) -> &HostPresenceTable {
        &self.table
    }

    pub(super) fn replace_webrtc_grants(
        &mut self,
        bindings: &[conduit_wire::SessionBinding],
    ) -> Result<(), String> {
        self.rendezvous
            .replace_grants(bindings)
            .map(|_| ())
            .map_err(|refusal| format!("replace browser WebRTC grants: {refusal:?}"))
    }

    pub(super) fn preflight_webrtc_grants(
        &self,
        bindings: &[conduit_wire::SessionBinding],
    ) -> Result<(), String> {
        self.rendezvous
            .preflight_grants(bindings)
            .map(|_| ())
            .map_err(|refusal| format!("preflight browser WebRTC grants: {refusal:?}"))
    }

    pub(super) fn deactivate_webrtc_grants(&mut self) {
        let _invalidated = self.rendezvous.deactivate_grants();
    }

    pub(super) fn is_running(&self) -> bool {
        !self.workers.is_empty()
    }

    pub(super) fn register(
        &mut self,
        socket: BrowserAdmissionSocket,
        credential: MembershipCredential,
        membership: &mut BodyMembership,
    ) -> Result<(), String> {
        if !self
            .credentials
            .iter()
            .any(|retained| retained.credential_id == credential.credential_id)
        {
            if self.credentials.len() == conduit_body::MAX_BODY_PARTS {
                return Err("browser presence credential capacity exhausted".into());
            }
            self.credentials.push(credential.clone());
        }
        self.register_at_sequence(socket, credential, membership, 1)
    }

    pub(super) fn return_identity(
        &self,
        credential: &MembershipCredential,
    ) -> Option<(&MembershipCredential, conduit_core::OfferGeneration)> {
        let retained = self
            .credentials
            .iter()
            .find(|retained| retained.credential_id == credential.credential_id)?;
        let lease = self
            .table
            .leases
            .iter()
            .find(|lease| lease.part_id == retained.part_id)?;
        Some((retained, lease.offer_generation))
    }

    fn register_at_sequence(
        &mut self,
        mut socket: BrowserAdmissionSocket,
        credential: MembershipCredential,
        membership: &mut BodyMembership,
        sequence: u64,
    ) -> Result<(), String> {
        if self.workers.len() == conduit_body::MAX_BODY_PARTS {
            return Err("browser presence worker capacity exhausted".into());
        }
        let observed_at_millis = self.now_millis()?;
        self.session_sequence = self
            .session_sequence
            .checked_add(1)
            .ok_or("browser presence session sequence exhausted")?;
        let session_id = LinkBindingId::from(format!(
            "patchbay/browser-presence/{}/{}",
            credential.credential_id.as_str(),
            self.session_sequence
        ));
        let started_sign = self.next_sign("started")?;
        self.table
            .start(
                membership,
                &credential.part_id,
                session_id.clone(),
                sequence,
                observed_at_millis,
                LEASE_MILLIS,
                started_sign,
            )
            .map_err(debug("start browser presence"))?;
        let expires_at_millis = self
            .table
            .leases
            .iter()
            .find(|lease| lease.part_id == credential.part_id)
            .expect("started lease is retained")
            .expires_at_millis;
        if let Err(error) = send_accepted(&mut socket, sequence, expires_at_millis) {
            let lost_at_millis = self.now_millis()?;
            let lost_sign = self.next_sign("initial-response-lost")?;
            self.table
                .lose_session(
                    membership,
                    &credential.part_id,
                    &session_id,
                    lost_at_millis,
                    lost_sign,
                )
                .map_err(debug("close failed initial presence session"))?;
            return Err(error);
        }
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .map_err(debug("set browser presence timeout"))?;
        let (receiver, outbound) = spawn_worker(socket);
        self.workers.push(PresenceWorker {
            credential,
            session_id,
            receiver,
            outbound,
        });
        Ok(())
    }

    pub(super) fn poll(
        &mut self,
        membership: &mut BodyMembership,
    ) -> Result<Option<String>, String> {
        let now = self.now_millis()?;
        if let Some(lease) = self
            .table
            .leases
            .iter()
            .find(|lease| {
                lease.state == HostPresenceState::Available && now >= lease.expires_at_millis
            })
            .cloned()
        {
            let expired_sign = self.next_sign("expired")?;
            self.table
                .expire(membership, &lease.part_id, now, expired_sign)
                .map_err(debug("expire browser presence"))?;
            self.workers
                .retain(|worker| worker.session_id != lease.session_binding_id);
            return Ok(Some(format!(
                "Browser Part {} is offline after its presence lease expired",
                lease.part_id.as_str()
            )));
        }
        for index in 0..self.workers.len() {
            let event = match self.workers[index].receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => continue,
                Err(TryRecvError::Disconnected) => WorkerEvent::Lost,
            };
            match event {
                WorkerEvent::Renewal { frame, response } => {
                    let frame = *frame;
                    return match &frame {
                        BrowserAdmissionIngress::PresenceRenewal { .. } => {
                            self.renew(index, frame, response, membership, now)
                        }
                        BrowserAdmissionIngress::WebRtcSignal { .. } => {
                            self.relay_webrtc(index, frame, response)
                        }
                        BrowserAdmissionIngress::WebRtcGrantRequest { .. } => {
                            self.provide_webrtc_grant(index, frame, response)
                        }
                        _ => {
                            let _ = response.send(WorkerResponse::Refused(
                                "unexpected-post-admission-frame".into(),
                            ));
                            self.lose(index, membership, now, "invalid-frame")
                        }
                    };
                }
                WorkerEvent::Lost => return self.lose(index, membership, now, "session-lost"),
                WorkerEvent::Failed(error) => {
                    let message = self.lose(index, membership, now, "transport-failed")?;
                    return Ok(message.map(|message| format!("{message}: {error}")));
                }
            }
        }
        Ok(None)
    }

    fn renew(
        &mut self,
        index: usize,
        frame: BrowserAdmissionIngress,
        response: SyncSender<WorkerResponse>,
        membership: &mut BodyMembership,
        now: u64,
    ) -> Result<Option<String>, String> {
        let BrowserAdmissionIngress::PresenceRenewal {
            credential_id,
            body_id,
            part_id,
            host_id,
            boot_id,
            sequence,
            ..
        } = frame
        else {
            let _ = response.send(WorkerResponse::Refused(
                "unexpected-post-admission-frame".into(),
            ));
            return self.lose(index, membership, now, "invalid-frame");
        };
        let credential = self.workers[index].credential.clone();
        let session_id = self.workers[index].session_id.clone();
        if credential_id != credential.credential_id
            || body_id != credential.body_id
            || part_id != credential.part_id
            || host_id != credential.host_id
            || boot_id != credential.boot_id
        {
            let _ = response.send(WorkerResponse::Refused(
                "stale-membership-credential".into(),
            ));
            return self.lose(index, membership, now, "identity-refused");
        }
        let renewed_sign = self.next_sign("renewed")?;
        if let Err(refusal) = self.table.renew(
            membership,
            &part_id,
            &session_id,
            sequence,
            now,
            LEASE_MILLIS,
            renewed_sign,
        ) {
            let _ = response.send(WorkerResponse::Refused(format!("presence-{refusal:?}")));
            return self.lose(index, membership, now, "renewal-refused");
        }
        let expires_at_millis = self
            .table
            .leases
            .iter()
            .find(|lease| lease.part_id == part_id)
            .expect("renewed lease is retained")
            .expires_at_millis;
        response
            .send(WorkerResponse::Accepted {
                sequence,
                expires_at_millis,
            })
            .map_err(|_| "browser presence response worker disconnected".to_string())?;
        Ok(Some(format!(
            "Browser Part {} presence renewed at sequence {sequence}",
            part_id.as_str()
        )))
    }

    fn lose(
        &mut self,
        index: usize,
        membership: &mut BodyMembership,
        now: u64,
        reason: &str,
    ) -> Result<Option<String>, String> {
        let worker = self.workers.swap_remove(index);
        let invalidated = self
            .rendezvous
            .invalidate(&worker.credential.host_id, &worker.credential.boot_id)
            .len();
        let lost_sign = self.next_sign(reason)?;
        self.table
            .lose_session(
                membership,
                &worker.credential.part_id,
                &worker.session_id,
                now,
                lost_sign,
            )
            .map_err(debug("lose browser presence session"))?;
        Ok(Some(format!(
            "Browser Part {} is offline; durable membership remains; invalidated WebRTC sessions={invalidated}",
            worker.credential.part_id.as_str()
        )))
    }

    fn now_millis(&self) -> Result<u64, String> {
        u64::try_from(self.clock.elapsed().as_millis())
            .map_err(|_| "browser presence clock overflowed".into())
    }

    fn next_sign(&mut self, label: &str) -> Result<SignId, String> {
        self.sign_sequence = self
            .sign_sequence
            .checked_add(1)
            .ok_or("browser presence Sign sequence exhausted")?;
        Ok(presence_sign(label, self.sign_sequence))
    }
}

fn presence_sign(label: &str, sequence: u64) -> SignId {
    SignId::from(format!("patchbay/browser-presence/{label}/{sequence}"))
}

fn spawn_worker(
    mut socket: BrowserAdmissionSocket,
) -> (Receiver<WorkerEvent>, SyncSender<BrowserAdmissionEgress>) {
    let (sender, receiver) = mpsc::sync_channel(1);
    let (outbound, outbound_receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || loop {
        match outbound_receiver.try_recv() {
            Ok(frame) => {
                if socket.send(&frame).is_err() {
                    let _ = sender.send(WorkerEvent::Lost);
                    return;
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return,
        }
        let frame = match socket.receive() {
            Ok(frame) => frame,
            Err(BrowserAdmissionSocketError::Transport(NativeWebSocketError::Transport(
                ErrorKind::WouldBlock | ErrorKind::TimedOut,
            ))) => continue,
            Err(BrowserAdmissionSocketError::Transport(NativeWebSocketError::Disconnected)) => {
                let _ = sender.send(WorkerEvent::Lost);
                return;
            }
            Err(error) => {
                let _ = sender.send(WorkerEvent::Failed(format!("{error:?}")));
                return;
            }
        };
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        if sender
            .send(WorkerEvent::Renewal {
                frame: Box::new(frame),
                response: response_sender,
            })
            .is_err()
        {
            return;
        }
        match response_receiver.recv() {
            Ok(WorkerResponse::Accepted {
                sequence,
                expires_at_millis,
            }) => {
                if send_accepted(&mut socket, sequence, expires_at_millis).is_err() {
                    let _ = sender.send(WorkerEvent::Lost);
                    return;
                }
            }
            Ok(WorkerResponse::Refused(code)) => {
                let _ = socket.send(&BrowserAdmissionEgress::Refused {
                    protocol: BROWSER_ADMISSION_PROTOCOL,
                    code,
                });
                return;
            }
            Ok(WorkerResponse::Relayed) => {}
            Ok(WorkerResponse::WebRtcGrant {
                index,
                total,
                grant,
            }) => {
                if socket
                    .send(&BrowserAdmissionEgress::WebRtcGrant {
                        protocol: BROWSER_ADMISSION_PROTOCOL,
                        index,
                        total,
                        grant,
                    })
                    .is_err()
                {
                    let _ = sender.send(WorkerEvent::Lost);
                    return;
                }
            }
            Err(_) => return,
        }
    });
    (receiver, outbound)
}

fn send_accepted(
    socket: &mut BrowserAdmissionSocket,
    sequence: u64,
    expires_at_millis: u64,
) -> Result<(), String> {
    socket
        .send(&BrowserAdmissionEgress::PresenceAccepted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            sequence,
            renew_after_millis: RENEW_AFTER_MILLIS,
            expires_at_millis,
        })
        .map_err(debug("send browser presence acceptance"))
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
