//! Explicit ambient-browser candidate admission for the active native Body.

use conduit_body::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, BodyId, BodyMembership, CandidateId,
    CandidateInventory, CandidateObservation, DiscoveryProofId,
};
use conduit_core::{LinkBindingId, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BrowserAdmissionSocket, BROWSER_ADMISSION_PROTOCOL,
};
use std::sync::mpsc::{self, Receiver, TryRecvError};

const CHALLENGE_LIFETIME_MILLIS: u64 = 60_000;

pub(super) struct AmbientBrowserCoordinator {
    body_id: BodyId,
    manager: AdmissionManager,
    accepting: Option<Receiver<AmbientAcceptOutcome>>,
    decisions: Vec<AwaitingDecision>,
    proofs: Vec<Receiver<Result<AmbientProofArrival, String>>>,
    returns: Vec<super::browser_return::ReturnArrival>,
}

enum AmbientArrival {
    Candidate {
        socket: BrowserAdmissionSocket,
        observation: CandidateObservation,
        verifying_key: [u8; 32],
    },
    Return(super::browser_return::ReturnArrival),
}

struct AmbientAcceptOutcome {
    listener: BrowserAdmissionListener,
    result: Result<AmbientArrival, String>,
}

struct AwaitingDecision {
    candidate_id: CandidateId,
    socket: BrowserAdmissionSocket,
    verifying_key: [u8; 32],
}

pub(super) struct AmbientProofArrival {
    socket: BrowserAdmissionSocket,
    proof: AmbientAdmissionProof,
}

pub(super) struct AdmittedAmbientBrowser {
    pub(super) socket: BrowserAdmissionSocket,
    pub(super) credential: conduit_body::MembershipCredential,
}

impl AmbientBrowserCoordinator {
    pub(super) fn start(body_id: BodyId) -> Result<(Self, String), String> {
        let manager = AdmissionManager::new(body_id.clone()).map_err(debug("manager"))?;
        let listener = BrowserAdmissionListener::bind_loopback().map_err(debug("bind"))?;
        let url = listener.url().map_err(debug("URL"))?;
        Ok((
            Self {
                body_id,
                manager,
                accepting: Some(spawn_accept(listener)),
                decisions: Vec::with_capacity(conduit_body::MAX_CANDIDATES),
                proofs: Vec::with_capacity(conduit_body::MAX_PENDING_ADMISSIONS),
                returns: Vec::with_capacity(conduit_body::MAX_PENDING_ADMISSIONS),
            },
            url,
        ))
    }

    pub(super) fn poll_candidate(
        &mut self,
        inventory: &mut CandidateInventory,
    ) -> Result<Option<CandidateId>, String> {
        let Some(receiver) = &self.accepting else {
            return Ok(None);
        };
        let outcome = match receiver.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.accepting = None;
                return Err("ambient browser candidate worker disconnected".into());
            }
        };
        self.accepting = Some(spawn_accept(outcome.listener));
        let arrival = outcome.result?;
        let AmbientArrival::Candidate {
            socket,
            observation,
            verifying_key,
        } = arrival
        else {
            if self.returns.len() == conduit_body::MAX_PENDING_ADMISSIONS {
                return Err("browser return capacity exhausted".into());
            }
            let AmbientArrival::Return(arrival) = arrival else {
                unreachable!()
            };
            self.returns.push(arrival);
            return Ok(None);
        };
        if self.decisions.len() == conduit_body::MAX_CANDIDATES {
            return Err("ambient browser decision capacity exhausted".into());
        }
        let candidate_id = inventory
            .observe(observation)
            .map_err(debug("observe candidate"))?;
        self.decisions.push(AwaitingDecision {
            candidate_id: candidate_id.clone(),
            socket,
            verifying_key,
        });
        Ok(Some(candidate_id))
    }

    pub(super) fn admit(
        &mut self,
        inventory: &mut CandidateInventory,
        candidate_id: &CandidateId,
        nonce: [u8; 32],
        now_millis: u64,
        requested: SignId,
    ) -> Result<(), String> {
        let index = self
            .decisions
            .iter()
            .position(|decision| &decision.candidate_id == candidate_id)
            .ok_or("ambient candidate has no live proof channel")?;
        if self.proofs.len() == conduit_body::MAX_PENDING_ADMISSIONS {
            return Err("ambient browser proof capacity exhausted".into());
        }
        let mut decision = self.decisions.swap_remove(index);
        let challenge = self
            .manager
            .begin_ambient(
                inventory,
                candidate_id,
                decision.verifying_key,
                nonce,
                now_millis,
                now_millis
                    .checked_add(CHALLENGE_LIFETIME_MILLIS)
                    .ok_or("ambient challenge expiry overflow")?,
                requested,
            )
            .map_err(debug("begin ambient admission"))?;
        decision
            .socket
            .send(&BrowserAdmissionEgress::Challenge {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                challenge,
            })
            .map_err(debug("send challenge"))?;
        self.proofs.push(spawn_proof(decision.socket));
        Ok(())
    }

    pub(super) fn take_proof(&mut self) -> Result<Option<AmbientProofArrival>, String> {
        for index in 0..self.proofs.len() {
            let result = match self.proofs[index].try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => continue,
                Err(TryRecvError::Disconnected) => {
                    self.proofs.swap_remove(index);
                    return Err("ambient browser proof worker disconnected".into());
                }
            };
            self.proofs.swap_remove(index);
            return result.map(Some);
        }
        Ok(None)
    }

    pub(super) fn take_return(&mut self) -> Option<super::browser_return::ReturnArrival> {
        self.returns.pop()
    }

    pub(super) fn manager_mut(&mut self) -> &mut AdmissionManager {
        &mut self.manager
    }

    pub(super) fn complete(
        &mut self,
        arrival: AmbientProofArrival,
        inventory: &mut CandidateInventory,
        membership: &mut BodyMembership,
        now_millis: u64,
        signs: AdmissionSigns,
    ) -> Result<AdmittedAmbientBrowser, String> {
        let mut socket = arrival.socket;
        match self.manager.complete_ambient(
            inventory,
            membership,
            &arrival.proof,
            now_millis,
            signs,
        ) {
            Ok(credential) => {
                socket
                    .send(&BrowserAdmissionEgress::Admitted {
                        protocol: BROWSER_ADMISSION_PROTOCOL,
                        credential: credential.clone(),
                    })
                    .map_err(debug("send admission"))?;
                Ok(AdmittedAmbientBrowser { socket, credential })
            }
            Err(refusal) => {
                let _ = socket.send(&BrowserAdmissionEgress::Refused {
                    protocol: BROWSER_ADMISSION_PROTOCOL,
                    code: format!("{refusal:?}"),
                });
                Err(format!("ambient browser admission refused: {refusal:?}"))
            }
        }
    }

    pub(super) fn refuse(&mut self, candidate_id: &CandidateId) {
        if let Some(index) = self
            .decisions
            .iter()
            .position(|decision| &decision.candidate_id == candidate_id)
        {
            let mut decision = self.decisions.swap_remove(index);
            let _ = decision.socket.send(&BrowserAdmissionEgress::Refused {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                code: "OperatorRefused".into(),
            });
        }
    }

    pub(super) fn is_running(&self) -> bool {
        self.accepting.is_some() || !self.proofs.is_empty() || !self.returns.is_empty()
    }

    pub(super) fn body_id(&self) -> &BodyId {
        &self.body_id
    }
}

fn spawn_accept(listener: BrowserAdmissionListener) -> Receiver<AmbientAcceptOutcome> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = receive_arrival(&listener);
        let _ = sender.send(AmbientAcceptOutcome { listener, result });
    });
    receiver
}

fn receive_arrival(listener: &BrowserAdmissionListener) -> Result<AmbientArrival, String> {
    let mut socket = listener.accept().map_err(debug("accept"))?;
    let (frame, encoded_bytes) = socket.receive_with_size().map_err(debug("advertisement"))?;
    let BrowserAdmissionIngress::Advertise {
        advertisement,
        friendly_label,
        verifying_key,
        freshness_sequence,
        ..
    } = frame
    else {
        if let BrowserAdmissionIngress::ReturnAdvertise {
            credential,
            advertisement,
            ..
        } = frame
        {
            return Ok(AmbientArrival::Return(
                super::browser_return::ReturnArrival {
                    socket,
                    credential,
                    advertisement,
                },
            ));
        }
        return Err("ambient browser did not begin with an advertisement or return".into());
    };
    let verifying_key = verifying_key
        .try_into()
        .map_err(|_| "invalid ambient browser verifying key")?;
    let observation = CandidateObservation {
        advertisement,
        friendly_label,
        observed_binding_id: LinkBindingId::from("line/browser-ambient/loopback"),
        observation_sign_id: SignId::from("sign/browser-ambient/observed"),
        proof_id: DiscoveryProofId::bind("proof/browser-ambient/loopback")
            .map_err(debug("discovery proof"))?,
        freshness_sequence,
        encoded_bytes,
    };
    Ok(AmbientArrival::Candidate {
        socket,
        observation,
        verifying_key,
    })
}

fn spawn_proof(
    mut socket: BrowserAdmissionSocket,
) -> Receiver<Result<AmbientProofArrival, String>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = receive_proof(&mut socket).map(|proof| AmbientProofArrival { socket, proof });
        let _ = sender.send(result);
    });
    receiver
}

fn receive_proof(socket: &mut BrowserAdmissionSocket) -> Result<AmbientAdmissionProof, String> {
    let BrowserAdmissionIngress::AmbientProof {
        admission_id,
        body_id,
        host_id,
        boot_id,
        nonce,
        signature,
        ..
    } = socket.receive().map_err(debug("ambient proof"))?
    else {
        return Err("ambient browser did not provide an admission proof".into());
    };
    Ok(AmbientAdmissionProof {
        admission_id,
        body_id,
        host_id,
        boot_id,
        nonce: nonce.try_into().map_err(|_| "invalid admission nonce")?,
        signature: signature
            .try_into()
            .map_err(|_| "invalid admission signature")?,
    })
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}

#[cfg(test)]
#[path = "browser_ambient_tests.rs"]
mod tests;
