//! Bounded same-incarnation browser Part return over a fresh accepted socket.

use conduit_body::{AdmissionManager, BodyMembership, MembershipCredential, PartReturnProof};
use conduit_core::{HostAdvertisement, OfferGeneration, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BrowserAdmissionSocket, BROWSER_ADMISSION_PROTOCOL,
};
use std::io::Read;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{SystemTime, UNIX_EPOCH};

const RETURN_CHALLENGE_LIFETIME_MILLIS: u64 = 60_000;

pub(super) struct ReturnCoordinator {
    spawn_arrivals: Vec<Receiver<Result<ReturnArrival, String>>>,
    proofs: Vec<PendingProof>,
    attempted_parts: Vec<conduit_body::PartId>,
    sign_sequence: u64,
}

#[derive(Clone, Copy)]
enum ReturnRoute {
    Ambient,
    Spawn,
}

struct PendingProof {
    receiver: Receiver<Result<ReturnProofArrival, String>>,
    route: ReturnRoute,
}

pub(super) struct ReturnArrival {
    pub(super) socket: BrowserAdmissionSocket,
    pub(super) credential: MembershipCredential,
    pub(super) advertisement: HostAdvertisement,
}

pub(super) struct ReturnProofArrival {
    pub(super) socket: BrowserAdmissionSocket,
    pub(super) credential: MembershipCredential,
    pub(super) advertisement: HostAdvertisement,
    proof: PartReturnProof,
}

impl ReturnCoordinator {
    pub(super) fn new() -> Self {
        Self {
            spawn_arrivals: Vec::with_capacity(conduit_body::MAX_BODY_PARTS),
            proofs: Vec::with_capacity(conduit_body::MAX_PENDING_ADMISSIONS),
            attempted_parts: Vec::with_capacity(conduit_body::MAX_BODY_PARTS),
            sign_sequence: 0,
        }
    }

    pub(super) fn listen(&mut self, listener: BrowserAdmissionListener) -> Result<(), String> {
        if self.spawn_arrivals.len() == conduit_body::MAX_BODY_PARTS {
            return Err("spawn browser return rendezvous capacity exhausted".into());
        }
        self.spawn_arrivals.push(accept_once(listener));
        Ok(())
    }

    pub(super) fn is_running(&self) -> bool {
        !self.spawn_arrivals.is_empty() || !self.proofs.is_empty()
    }

    pub(super) fn poll(
        &mut self,
        membership: &mut BodyMembership,
        ambient: &mut Option<super::browser_ambient::AmbientBrowserCoordinator>,
        spawn_manager: &mut Option<AdmissionManager>,
        presence: &mut Option<super::browser_presence::BrowserPresenceCoordinator>,
    ) -> Result<Option<String>, String> {
        let ambient_arrival = ambient
            .as_mut()
            .and_then(super::browser_ambient::AmbientBrowserCoordinator::take_return)
            .map(|arrival| (arrival, ReturnRoute::Ambient));
        let arrival = match ambient_arrival {
            Some(arrival) => Some(arrival),
            None => self
                .take_spawn_arrival()?
                .map(|arrival| (arrival, ReturnRoute::Spawn)),
        };
        if let Some((arrival, route)) = arrival {
            return self.begin_arrival(
                membership,
                ambient,
                spawn_manager,
                presence,
                arrival,
                route,
            );
        }
        self.complete_ready(membership, ambient, spawn_manager, presence)
    }

    fn begin_arrival(
        &mut self,
        membership: &BodyMembership,
        ambient: &mut Option<super::browser_ambient::AmbientBrowserCoordinator>,
        spawn_manager: &mut Option<AdmissionManager>,
        presence: &Option<super::browser_presence::BrowserPresenceCoordinator>,
        arrival: ReturnArrival,
        route: ReturnRoute,
    ) -> Result<Option<String>, String> {
        if self.attempted_parts.contains(&arrival.credential.part_id) {
            reject(arrival, "return-attempt-exhausted")?;
            return Err("browser Part already consumed its one return attempt".into());
        }
        if self.proofs.len() == conduit_body::MAX_PENDING_ADMISSIONS {
            reject(arrival, "return-capacity-exhausted")?;
            return Err("browser return proof capacity exhausted".into());
        }
        let Some((expected, offer_generation)) = presence
            .as_ref()
            .and_then(|presence| presence.return_identity(&arrival.credential))
            .map(|(credential, offer)| (credential.clone(), offer))
        else {
            reject(arrival, "unknown-return-credential")?;
            return Err("browser return used an unknown retained credential".into());
        };
        let mut nonce = [0; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut nonce))
            .map_err(|error| format!("cannot obtain browser return entropy: {error}"))?;
        let part_id = expected.part_id.clone();
        let receiver = begin(
            manager_mut(route, ambient, spawn_manager)?,
            membership,
            arrival,
            &expected,
            offer_generation,
            nonce,
            now_millis()?,
        )?;
        self.attempted_parts.push(part_id);
        self.proofs.push(PendingProof { receiver, route });
        Ok(Some(format!(
            "Browser Part {} is proving exact return continuity",
            expected.part_id.as_str()
        )))
    }

    fn complete_ready(
        &mut self,
        membership: &mut BodyMembership,
        ambient: &mut Option<super::browser_ambient::AmbientBrowserCoordinator>,
        spawn_manager: &mut Option<AdmissionManager>,
        presence: &mut Option<super::browser_presence::BrowserPresenceCoordinator>,
    ) -> Result<Option<String>, String> {
        for index in 0..self.proofs.len() {
            let result = match self.proofs[index].receiver.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => continue,
                Err(TryRecvError::Disconnected) => {
                    self.proofs.swap_remove(index);
                    return Err("browser return proof worker disconnected".into());
                }
            };
            let route = self.proofs.swap_remove(index).route;
            let proof = result?;
            let presence = presence
                .as_mut()
                .ok_or("browser presence coordinator is absent")?;
            let (part_id, sequence) = complete_with_presence(
                manager_mut(route, ambient, spawn_manager)?,
                membership,
                presence,
                proof,
                now_millis()?,
                &mut self.sign_sequence,
            )?;
            return Ok(Some(format!(
                "Browser Part {} returned with fresh presence sequence {}",
                part_id.as_str(),
                sequence
            )));
        }
        Ok(None)
    }

    fn take_spawn_arrival(&mut self) -> Result<Option<ReturnArrival>, String> {
        for index in 0..self.spawn_arrivals.len() {
            match self.spawn_arrivals[index].try_recv() {
                Ok(result) => {
                    self.spawn_arrivals.swap_remove(index);
                    return result.map(Some);
                }
                Err(TryRecvError::Disconnected) => {
                    self.spawn_arrivals.swap_remove(index);
                    return Err("spawn browser return listener disconnected".into());
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        Ok(None)
    }

    #[cfg(test)]
    pub(super) fn atomic_state_for_test(&self) -> (usize, u64) {
        (self.attempted_parts.len(), self.sign_sequence)
    }
}

pub(super) fn accept_once(
    listener: BrowserAdmissionListener,
) -> Receiver<Result<ReturnArrival, String>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = receive_return(&listener);
        let _ = sender.send(result);
    });
    receiver
}

pub(super) fn reject(mut arrival: ReturnArrival, code: &str) -> Result<(), String> {
    refuse(&mut arrival.socket, code)
}

pub(super) fn begin(
    manager: &mut AdmissionManager,
    membership: &BodyMembership,
    mut arrival: ReturnArrival,
    expected: &MembershipCredential,
    expected_offer_generation: OfferGeneration,
    nonce: [u8; 32],
    now_millis: u64,
) -> Result<Receiver<Result<ReturnProofArrival, String>>, String> {
    if &arrival.credential != expected {
        let _ = refuse(&mut arrival.socket, "stale-membership-credential");
        return Err("browser return used a stale membership credential".into());
    }
    if arrival.advertisement.host_id != expected.host_id
        || arrival.advertisement.boot_id != expected.boot_id
        || arrival.advertisement.offer_generation != expected_offer_generation
    {
        let _ = refuse(&mut arrival.socket, "stale-return-advertisement");
        return Err("browser return advertisement changed exact Host truth".into());
    }
    let expires_at_millis = now_millis
        .checked_add(RETURN_CHALLENGE_LIFETIME_MILLIS)
        .ok_or("browser return challenge expiry overflow")?;
    let challenge = match manager.begin_return(
        membership,
        &expected.part_id,
        &arrival.advertisement,
        nonce,
        now_millis,
        expires_at_millis,
    ) {
        Ok(challenge) => challenge,
        Err(error) => {
            let _ = refuse(&mut arrival.socket, "return-not-admissible");
            return Err(format!("begin browser return: {error:?}"));
        }
    };
    arrival
        .socket
        .send(&BrowserAdmissionEgress::ReturnChallenge {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            challenge,
        })
        .map_err(debug("send browser return challenge"))?;
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = receive_proof(arrival);
        let _ = sender.send(result);
    });
    Ok(receiver)
}

pub(super) fn complete_with_presence(
    manager: &mut AdmissionManager,
    membership: &mut BodyMembership,
    presence: &mut super::browser_presence::BrowserPresenceCoordinator,
    mut arrival: ReturnProofArrival,
    now_millis: u64,
    sign_sequence: &mut u64,
) -> Result<(conduit_body::PartId, u64), String> {
    let next_sign_sequence = sign_sequence
        .checked_add(1)
        .ok_or("browser return Sign sequence exhausted")?;
    let attached_sign = SignId::from(format!(
        "patchbay/browser-return/attached/{next_sign_sequence}"
    ));
    let mut next_manager = manager.clone();
    let mut next_membership = membership.clone();
    let returned_credential = match next_manager.complete_return(
        &mut next_membership,
        &arrival.advertisement,
        &arrival.proof,
        now_millis,
        attached_sign,
    ) {
        Ok(credential) => credential,
        Err(error) => {
            let _ = refuse(&mut arrival.socket, "return-proof-refused");
            return Err(format!("complete browser return: {error:?}"));
        }
    };
    let prepared = match presence.prepare_return(
        &arrival.credential,
        &returned_credential,
        &next_membership,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = refuse(&mut arrival.socket, "return-presence-not-admissible");
            return Err(error);
        }
    };
    if let Err(error) = presence.prepare_return_socket(&arrival.socket) {
        let _ = refuse(&mut arrival.socket, "return-session-not-admissible");
        return Err(error);
    }
    let part_id = returned_credential.part_id.clone();
    *sign_sequence = next_sign_sequence;
    *manager = next_manager;
    *membership = next_membership;
    let sequence =
        presence.commit_return(arrival.socket, returned_credential, membership, prepared)?;
    Ok((part_id, sequence))
}

fn receive_proof(mut arrival: ReturnArrival) -> Result<ReturnProofArrival, String> {
    let frame = match arrival.socket.receive() {
        Ok(frame) => frame,
        Err(error) => {
            let _ = refuse(&mut arrival.socket, "return-proof-unavailable");
            return Err(format!("receive browser return proof: {error:?}"));
        }
    };
    let BrowserAdmissionIngress::ReturnProof {
        admission_id,
        body_id,
        part_id,
        host_id,
        boot_id,
        nonce,
        signature,
        ..
    } = frame
    else {
        let _ = refuse(&mut arrival.socket, "invalid-return-proof");
        return Err("returning browser did not provide exact continuity proof".into());
    };
    let nonce = match nonce.try_into() {
        Ok(nonce) => nonce,
        Err(_) => {
            let _ = refuse(&mut arrival.socket, "invalid-return-nonce");
            return Err("invalid return nonce".into());
        }
    };
    let signature = match signature.try_into() {
        Ok(signature) => signature,
        Err(_) => {
            let _ = refuse(&mut arrival.socket, "invalid-return-signature");
            return Err("invalid return signature".into());
        }
    };
    Ok(ReturnProofArrival {
        socket: arrival.socket,
        credential: arrival.credential,
        advertisement: arrival.advertisement,
        proof: PartReturnProof {
            admission_id,
            body_id,
            part_id,
            host_id,
            boot_id,
            nonce,
            signature,
        },
    })
}

fn receive_return(listener: &BrowserAdmissionListener) -> Result<ReturnArrival, String> {
    let mut socket = listener.accept().map_err(debug("accept browser return"))?;
    let frame = socket
        .receive()
        .map_err(debug("browser return advertisement"))?;
    let BrowserAdmissionIngress::ReturnAdvertise {
        credential,
        advertisement,
        ..
    } = frame
    else {
        let _ = refuse(&mut socket, "return-advertisement-required");
        return Err("browser return rendezvous accepts only ReturnAdvertise".into());
    };
    Ok(ReturnArrival {
        socket,
        credential,
        advertisement,
    })
}

fn refuse(socket: &mut BrowserAdmissionSocket, code: &str) -> Result<(), String> {
    socket
        .send(&BrowserAdmissionEgress::Refused {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            code: code.into(),
        })
        .map_err(debug("send browser return refusal"))
}

fn manager_mut<'a>(
    route: ReturnRoute,
    ambient: &'a mut Option<super::browser_ambient::AmbientBrowserCoordinator>,
    spawn_manager: &'a mut Option<AdmissionManager>,
) -> Result<&'a mut AdmissionManager, String> {
    match route {
        ReturnRoute::Ambient => ambient
            .as_mut()
            .map(super::browser_ambient::AmbientBrowserCoordinator::manager_mut)
            .ok_or_else(|| "ambient browser return manager is absent".into()),
        ReturnRoute::Spawn => spawn_manager
            .as_mut()
            .ok_or_else(|| "spawn browser return manager is absent".into()),
    }
}

fn now_millis() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock before epoch: {error}"))?
        .as_millis();
    u64::try_from(millis).map_err(|_| "system clock exceeds Body admission range".into())
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
