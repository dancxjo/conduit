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
}

struct AmbientArrival {
    socket: BrowserAdmissionSocket,
    observation: CandidateObservation,
    verifying_key: [u8; 32],
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
        if self.decisions.len() == conduit_body::MAX_CANDIDATES {
            return Err("ambient browser decision capacity exhausted".into());
        }
        let candidate_id = inventory
            .observe(arrival.observation)
            .map_err(debug("observe candidate"))?;
        self.decisions.push(AwaitingDecision {
            candidate_id: candidate_id.clone(),
            socket: arrival.socket,
            verifying_key: arrival.verifying_key,
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

    pub(super) fn complete(
        &mut self,
        arrival: AmbientProofArrival,
        inventory: &mut CandidateInventory,
        membership: &mut BodyMembership,
        now_millis: u64,
        signs: AdmissionSigns,
    ) -> Result<conduit_body::MembershipCredential, String> {
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
                Ok(credential)
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
        self.accepting.is_some() || !self.proofs.is_empty()
    }

    pub(super) fn body_id(&self) -> &BodyId {
        &self.body_id
    }
}

fn spawn_accept(listener: BrowserAdmissionListener) -> Receiver<AmbientAcceptOutcome> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = receive_candidate(&listener).map(|(socket, observation, verifying_key)| {
            AmbientArrival {
                socket,
                observation,
                verifying_key,
            }
        });
        let _ = sender.send(AmbientAcceptOutcome { listener, result });
    });
    receiver
}

fn receive_candidate(
    listener: &BrowserAdmissionListener,
) -> Result<(BrowserAdmissionSocket, CandidateObservation, [u8; 32]), String> {
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
        return Err("ambient browser did not begin with an advertisement".into());
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
    Ok((socket, observation, verifying_key))
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
mod tests {
    use super::*;
    use conduit_browser_runtime::membership::BrowserAdmissionIdentity;
    use conduit_core::{
        BootId, CheckedFormId, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
        SourceDocumentId,
    };
    use conduit_std_host::{
        browser_admission::MAX_BROWSER_ADMISSION_FRAME_BYTES, websocket::NativeWebSocketLine,
    };
    use std::net::SocketAddr;
    use std::time::{Duration, Instant};

    #[test]
    fn ambient_page_stays_candidate_until_explicit_admit_completes_exact_proof() {
        let body_id = conduit_body::Body::born(
            SourceDocumentId::from("source/native-ambient-test"),
            CheckedFormId::from("checked/native-ambient-test"),
            1,
            SignId::from("sign/native-ambient/body"),
        )
        .unwrap()
        .body_id;
        let (mut coordinator, url) = AmbientBrowserCoordinator::start(body_id.clone()).unwrap();
        let identity = BrowserAdmissionIdentity::from_csprng_seed(
            HostId::from("browser/native-ambient"),
            BootId::from("browser-boot/native-ambient"),
            [7; 32],
        )
        .unwrap();
        let advertisement = HostAdvertisement {
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: identity.host_id().clone(),
            boot_id: identity.boot_id().clone(),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("browser/host"),
            resources: Vec::new(),
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        };
        let client_url = url.clone();
        let client = std::thread::spawn(move || {
            let address: SocketAddr = client_url
                .strip_prefix("ws://")
                .unwrap()
                .strip_suffix("/conduit")
                .unwrap()
                .parse()
                .unwrap();
            let mut line = NativeWebSocketLine::connect(
                address,
                &client_url,
                MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
            )
            .unwrap();
            let advertise = BrowserAdmissionIngress::Advertise {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                advertisement,
                friendly_label: "This computer".into(),
                verifying_key: identity.verifying_key().to_vec(),
                freshness_sequence: 1,
            };
            let mut encoded = [0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
            let bytes = serde_json::to_vec(&advertise).unwrap();
            line.send_binary(&bytes).unwrap();
            let length = line.receive_binary(&mut encoded).unwrap();
            let frame: BrowserAdmissionEgress = serde_json::from_slice(&encoded[..length]).unwrap();
            let BrowserAdmissionEgress::Challenge { challenge, .. } = frame else {
                panic!("explicit Admit must send a challenge");
            };
            let proof = identity.prove(&challenge).unwrap();
            let proof = BrowserAdmissionIngress::AmbientProof {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                admission_id: proof.admission_id,
                body_id: proof.body_id,
                host_id: proof.host_id,
                boot_id: proof.boot_id,
                nonce: proof.nonce.to_vec(),
                signature: proof.signature.to_vec(),
            };
            let bytes = serde_json::to_vec(&proof).unwrap();
            line.send_binary(&bytes).unwrap();
            let length = line.receive_binary(&mut encoded).unwrap();
            serde_json::from_slice::<BrowserAdmissionEgress>(&encoded[..length]).unwrap()
        });

        let mut inventory = CandidateInventory::new(body_id.clone()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let candidate_id = loop {
            if let Some(candidate) = coordinator.poll_candidate(&mut inventory).unwrap() {
                break candidate;
            }
            assert!(Instant::now() < deadline, "candidate did not arrive");
            std::thread::yield_now();
        };
        assert_eq!(inventory.candidates.len(), 1);
        assert_eq!(
            inventory.candidates[0].state,
            conduit_body::CandidateState::Discovered
        );
        let mut membership = BodyMembership::new(body_id).unwrap();
        assert!(membership.parts.is_empty());
        coordinator
            .admit(
                &mut inventory,
                &candidate_id,
                [9; 32],
                1_000,
                SignId::from("sign/native-ambient/requested"),
            )
            .unwrap();
        let arrival = loop {
            if let Some(arrival) = coordinator.take_proof().unwrap() {
                break arrival;
            }
            assert!(Instant::now() < deadline, "proof did not arrive");
            std::thread::yield_now();
        };
        let credential = coordinator
            .complete(
                arrival,
                &mut inventory,
                &mut membership,
                1_001,
                AdmissionSigns {
                    part_admitted: SignId::from("sign/native-ambient/part"),
                    host_attached: SignId::from("sign/native-ambient/host"),
                    candidate_admitted: SignId::from("sign/native-ambient/candidate"),
                },
            )
            .unwrap();
        assert_eq!(membership.parts.len(), 1);
        assert_eq!(membership.parts[0].part_id, credential.part_id);
        assert!(matches!(
            client.join().unwrap(),
            BrowserAdmissionEgress::Admitted { .. }
        ));
    }
}
