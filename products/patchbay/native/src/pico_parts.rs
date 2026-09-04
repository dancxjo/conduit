//! Explicit provisioned-Pico admission for the active native Body.

use conduit_body::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, BodyId, BodyMembership, CandidateId,
    CandidateInventory, DiscoveryProofId,
};
use conduit_core::{LinkBindingId, SignId};
use conduit_std_host::pico_admission::{PicoAdmissionArrival, PicoAdmissionSocket};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

const IO_TIMEOUT: Duration = Duration::from_secs(10);
const CHALLENGE_LIFETIME_MILLIS: u64 = 60_000;

pub(super) struct PicoPartsCoordinator {
    manager: AdmissionManager,
    observation: Option<Receiver<Result<PicoAdmissionArrival, String>>>,
    awaiting: Option<AwaitingDecision>,
    proof: Option<Receiver<Result<PicoProofArrival, String>>>,
    attached: Option<AttachedPico>,
}

struct AwaitingDecision {
    candidate_id: CandidateId,
    socket: PicoAdmissionSocket,
    verifying_key: [u8; 32],
}

pub(super) struct PicoProofArrival {
    proof: AmbientAdmissionProof,
    socket: PicoAdmissionSocket,
}

struct AttachedPico {
    part_id: conduit_body::PartId,
    boot_id: conduit_core::BootId,
    socket: PicoAdmissionSocket,
}

impl PicoPartsCoordinator {
    pub(super) fn start(body_id: BodyId, path: String) -> Result<Self, String> {
        let manager = AdmissionManager::new(body_id).map_err(debug("manager"))?;
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = PicoAdmissionSocket::open(&path)
                .map_err(debug("open Pico Line"))
                .and_then(|socket| {
                    socket
                        .observe(
                            LinkBindingId::from(format!("pico/usb-cdc/{path}")),
                            SignId::from("patchbay-native/pico-observed"),
                            DiscoveryProofId::bind("patchbay-native/pico-usb-observation")
                                .map_err(debug("discovery proof"))?,
                            IO_TIMEOUT,
                        )
                        .map_err(debug("observe Pico"))
                });
            let _ = sender.send(result);
        });
        Ok(Self {
            manager,
            observation: Some(receiver),
            awaiting: None,
            proof: None,
            attached: None,
        })
    }

    pub(super) fn poll_candidate(
        &mut self,
        inventory: &mut CandidateInventory,
    ) -> Result<Option<CandidateId>, String> {
        let Some(receiver) = &self.observation else {
            return Ok(None);
        };
        let arrival = match receiver.try_recv() {
            Ok(result) => result?,
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.observation = None;
                return Err("Pico observation worker disconnected".into());
            }
        };
        self.observation = None;
        let candidate_id = inventory
            .observe(arrival.observation)
            .map_err(debug("observe Pico candidate"))?;
        self.awaiting = Some(AwaitingDecision {
            candidate_id: candidate_id.clone(),
            socket: arrival.socket,
            verifying_key: arrival.verifying_key,
        });
        Ok(Some(candidate_id))
    }

    pub(super) fn owns(&self, candidate_id: &CandidateId) -> bool {
        self.awaiting
            .as_ref()
            .is_some_and(|awaiting| &awaiting.candidate_id == candidate_id)
    }

    pub(super) fn is_running(&self) -> bool {
        self.observation.is_some() || self.proof.is_some() || self.attached.is_some()
    }

    pub(super) fn admit(
        &mut self,
        inventory: &mut CandidateInventory,
        candidate_id: &CandidateId,
        nonce: [u8; 32],
        now_millis: u64,
        requested: SignId,
    ) -> Result<(), String> {
        let awaiting = self
            .awaiting
            .take()
            .filter(|awaiting| &awaiting.candidate_id == candidate_id)
            .ok_or("Pico candidate has no live proof channel")?;
        let challenge = self
            .manager
            .begin_ambient(
                inventory,
                candidate_id,
                awaiting.verifying_key,
                nonce,
                now_millis,
                now_millis
                    .checked_add(CHALLENGE_LIFETIME_MILLIS)
                    .ok_or("Pico challenge expiry overflow")?,
                requested,
            )
            .map_err(debug("begin Pico admission"))?;
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = awaiting
                .socket
                .prove(&challenge, IO_TIMEOUT)
                .map(|(proof, socket)| PicoProofArrival { proof, socket })
                .map_err(debug("receive Pico proof"));
            let _ = sender.send(result);
        });
        self.proof = Some(receiver);
        Ok(())
    }

    pub(super) fn take_proof(&mut self) -> Result<Option<PicoProofArrival>, String> {
        let Some(receiver) = &self.proof else {
            return Ok(None);
        };
        match receiver.try_recv() {
            Ok(result) => {
                self.proof = None;
                result.map(Some)
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.proof = None;
                Err("Pico proof worker disconnected".into())
            }
        }
    }

    pub(super) fn complete(
        &mut self,
        arrival: PicoProofArrival,
        inventory: &mut CandidateInventory,
        membership: &mut BodyMembership,
        now_millis: u64,
        signs: AdmissionSigns,
    ) -> Result<conduit_body::MembershipCredential, String> {
        let credential = self
            .manager
            .complete_ambient(inventory, membership, &arrival.proof, now_millis, signs)
            .map_err(debug("complete Pico admission"))?;
        self.attached = Some(AttachedPico {
            part_id: credential.part_id.clone(),
            boot_id: credential.boot_id.clone(),
            socket: arrival.socket,
        });
        Ok(credential)
    }

    pub(super) fn take_disconnect(
        &mut self,
    ) -> Result<Option<(conduit_body::PartId, conduit_core::BootId)>, String> {
        let Some(attached) = &self.attached else {
            return Ok(None);
        };
        if attached
            .socket
            .is_connected()
            .map_err(debug("poll Pico Line"))?
        {
            return Ok(None);
        }
        let attached = self.attached.take().expect("attached Pico checked");
        Ok(Some((attached.part_id, attached.boot_id)))
    }

    pub(super) fn refuse(&mut self, candidate_id: &CandidateId) {
        if self.owns(candidate_id) {
            self.awaiting = None;
        }
    }
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
