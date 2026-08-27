//! Native coordination for one Body-directed browser Part spawn.

use conduit_body::{
    AdmissionManager, AdmissionSigns, BodyId, BodyMembership, SpawnAdmissionProof,
    SpawnInvitationSecret,
};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BrowserAdmissionSocket, BROWSER_ADMISSION_PROTOCOL,
};
use std::io::Read;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{SystemTime, UNIX_EPOCH};

const INVITATION_LIFETIME_MILLIS: u64 = 60_000;

pub(super) struct BrowserPartsCoordinator {
    page_url: String,
    chat_url: String,
    manager: Option<AdmissionManager>,
    pending: Option<PendingSpawn>,
    ambient: Option<super::browser_ambient::AmbientBrowserCoordinator>,
    presence: Option<super::browser_presence::BrowserPresenceCoordinator>,
    returns: super::browser_return::ReturnCoordinator,
}

struct PendingSpawn {
    receiver: Receiver<Result<SpawnArrival, String>>,
    expires_at_millis: u64,
}

pub(super) struct SpawnArrival {
    listener: BrowserAdmissionListener,
    socket: BrowserAdmissionSocket,
    advertisement: conduit_core::HostAdvertisement,
    proof: SpawnAdmissionProof,
}

impl BrowserPartsCoordinator {
    pub(super) fn preflight_webrtc_grants(&self, plan: &conduit_core::Plan) -> Result<(), String> {
        let bindings = planned_webrtc_bindings(plan)?;
        match self.presence.as_ref() {
            Some(presence) => presence.preflight_webrtc_grants(&bindings),
            None if bindings.is_empty() => Ok(()),
            None => Err("planned browser WebRTC grants require browser presence".into()),
        }
    }

    pub(super) fn replace_webrtc_grants(
        &mut self,
        plan: &conduit_core::Plan,
    ) -> Result<(), String> {
        let bindings = planned_webrtc_bindings(plan)?;
        match self.presence.as_mut() {
            Some(presence) => presence.replace_webrtc_grants(&bindings),
            None if bindings.is_empty() => Ok(()),
            None => Err("planned browser WebRTC grants require browser presence".into()),
        }
    }

    pub(super) fn deactivate_webrtc_grants(&mut self) {
        if let Some(presence) = &mut self.presence {
            presence.deactivate_webrtc_grants();
        }
    }

    pub(super) fn new(page_url: String, chat_url: String) -> Self {
        Self {
            page_url,
            chat_url,
            manager: None,
            pending: None,
            ambient: None,
            presence: None,
            returns: super::browser_return::ReturnCoordinator::new(),
        }
    }

    pub(super) fn start_ambient(&mut self, body_id: &BodyId) -> Result<String, String> {
        self.ensure_presence(body_id)?;
        if let Some(ambient) = &self.ambient {
            if ambient.body_id() != body_id {
                return Err("ambient browser coordinator belongs to a different Body".into());
            }
            return Err("ambient browser discovery is already running".into());
        }
        let (ambient, body_url) =
            super::browser_ambient::AmbientBrowserCoordinator::start(body_id.clone())?;
        self.ambient = Some(ambient);
        Ok(format!(
            "{}?ws={}&body={}",
            self.page_url,
            percent_encode(&self.chat_url),
            percent_encode(&body_url)
        ))
    }

    pub(super) fn ambient_mut(
        &mut self,
    ) -> Option<&mut super::browser_ambient::AmbientBrowserCoordinator> {
        self.ambient.as_mut()
    }

    pub(super) fn begin(&mut self, body_id: &BodyId) -> Result<String, String> {
        self.ensure_presence(body_id)?;
        if self.pending.is_some() {
            return Err("a browser Part spawn is already pending".into());
        }
        if self
            .manager
            .as_ref()
            .is_some_and(|manager| &manager.body_id != body_id)
        {
            return Err("browser Part coordinator belongs to a different Body".into());
        }
        if self.manager.is_none() {
            self.manager = Some(AdmissionManager::new(body_id.clone()).map_err(debug("manager"))?);
        }
        let mut entropy = [0; 64];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut entropy))
            .map_err(|error| format!("cannot obtain browser invitation entropy: {error}"))?;
        let now = now_millis()?;
        let invitation = self
            .manager
            .as_mut()
            .expect("manager initialized")
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes(entropy[..32].try_into().unwrap())
                    .map_err(debug("secret"))?,
                entropy[32..].try_into().unwrap(),
                now,
                now.checked_add(INVITATION_LIFETIME_MILLIS)
                    .ok_or("browser invitation expiry overflow")?,
            )
            .map_err(debug("invitation"))?;
        let listener = BrowserAdmissionListener::bind_loopback().map_err(debug("bind"))?;
        let body_url = listener.url().map_err(debug("URL"))?;
        let envelope = serde_json::to_vec(&serde_json::json!({
            "claim": invitation.claim(),
            "secret": &entropy[..32],
        }))
        .map_err(|error| format!("encode browser invitation: {error}"))?;
        entropy.fill(0);
        let envelope_hex = envelope
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let target = spawn_target(&self.page_url, &self.chat_url, &body_url, &envelope_hex);
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = sender.send(receive_spawn(listener));
        });
        self.pending = Some(PendingSpawn {
            receiver,
            expires_at_millis: invitation.claim().expires_at_millis,
        });
        Ok(target)
    }

    pub(super) fn take_arrival(&mut self) -> Result<Option<SpawnArrival>, String> {
        let Some(pending) = &self.pending else {
            return Ok(None);
        };
        if now_millis()? > pending.expires_at_millis {
            self.pending = None;
            return Err("browser Part invitation expired".into());
        }
        match pending.receiver.try_recv() {
            Ok(result) => {
                self.pending = None;
                result.map(Some)
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.pending = None;
                Err("browser Part spawn worker disconnected".into())
            }
        }
    }

    pub(super) const fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn is_running(&self) -> bool {
        self.pending.is_some()
            || self
                .ambient
                .as_ref()
                .is_some_and(super::browser_ambient::AmbientBrowserCoordinator::is_running)
            || self
                .presence
                .as_ref()
                .is_some_and(super::browser_presence::BrowserPresenceCoordinator::is_running)
            || self.returns.is_running()
    }

    pub(super) fn cancel(&mut self) -> bool {
        self.pending.take().is_some()
    }

    pub(super) fn complete(
        &mut self,
        mut arrival: SpawnArrival,
        membership: &mut BodyMembership,
        signs: AdmissionSigns,
    ) -> Result<conduit_body::MembershipCredential, String> {
        let result = self
            .manager
            .as_mut()
            .ok_or("browser Part admission manager is absent")?
            .complete_spawn(
                membership,
                &arrival.advertisement,
                &arrival.proof,
                now_millis()?,
                signs,
            );
        match result {
            Ok(credential) => {
                arrival
                    .socket
                    .send(&BrowserAdmissionEgress::Admitted {
                        protocol: BROWSER_ADMISSION_PROTOCOL,
                        credential: credential.clone(),
                    })
                    .map_err(debug("send admission"))?;
                self.presence
                    .as_mut()
                    .expect("browser presence initialized with spawn")
                    .register(arrival.socket, credential.clone(), membership)?;
                self.returns.listen(arrival.listener)?;
                Ok(credential)
            }
            Err(refusal) => {
                let _ = arrival.socket.send(&BrowserAdmissionEgress::Refused {
                    protocol: BROWSER_ADMISSION_PROTOCOL,
                    code: format!("{refusal:?}"),
                });
                Err(format!("browser Part admission refused: {refusal:?}"))
            }
        }
    }

    pub(super) fn register_ambient_presence(
        &mut self,
        admitted: super::browser_ambient::AdmittedAmbientBrowser,
        membership: &mut BodyMembership,
    ) -> Result<conduit_body::MembershipCredential, String> {
        let credential = admitted.credential;
        self.presence
            .as_mut()
            .ok_or("browser presence coordinator is absent")?
            .register(admitted.socket, credential.clone(), membership)?;
        Ok(credential)
    }

    pub(super) fn poll_presence(
        &mut self,
        membership: &mut BodyMembership,
    ) -> Result<Option<String>, String> {
        self.presence
            .as_mut()
            .map(|presence| presence.poll(membership))
            .transpose()
            .map(Option::flatten)
    }

    pub(super) fn poll_return(
        &mut self,
        membership: &mut BodyMembership,
    ) -> Result<Option<String>, String> {
        self.returns.poll(
            membership,
            &mut self.ambient,
            &mut self.manager,
            &mut self.presence,
        )
    }

    pub(super) fn presence(&self) -> Option<&conduit_body::HostPresenceTable> {
        self.presence
            .as_ref()
            .map(super::browser_presence::BrowserPresenceCoordinator::table)
    }

    pub(super) fn presence_presentation_reference(
        &self,
    ) -> Result<Option<conduit_presentation::TemporalReference>, String> {
        self.presence
            .as_ref()
            .map(super::browser_presence::BrowserPresenceCoordinator::presentation_reference)
            .transpose()
    }

    #[cfg(test)]
    pub(super) fn observe_for_presentation_test(
        &mut self,
        body_id: &BodyId,
        membership: &BodyMembership,
        part_id: &conduit_body::PartId,
    ) -> Result<conduit_core::SignId, String> {
        self.ensure_presence(body_id)?;
        self.presence
            .as_mut()
            .expect("test presence was initialized")
            .observe_for_presentation_test(membership, part_id)
    }

    fn ensure_presence(&mut self, body_id: &BodyId) -> Result<(), String> {
        if let Some(presence) = &self.presence {
            if &presence.table().body_id != body_id {
                return Err("browser presence coordinator belongs to a different Body".into());
            }
        } else {
            self.presence = Some(super::browser_presence::BrowserPresenceCoordinator::new(
                body_id.clone(),
            )?);
        }
        Ok(())
    }

    #[cfg(test)]
    fn inject_return_preflight_fault_for_test(
        &mut self,
        part_id: &conduit_body::PartId,
        credential: &conduit_body::MembershipCredential,
        fault: tests::ReturnPreflightFault,
    ) {
        let presence = self
            .presence
            .as_mut()
            .expect("test browser presence exists");
        match fault {
            tests::ReturnPreflightFault::SequenceOverflow => {
                presence.set_return_sequence_for_test(part_id, u64::MAX);
            }
            tests::ReturnPreflightFault::AvailableLease => {
                presence.make_return_lease_available_for_test(part_id);
            }
            tests::ReturnPreflightFault::DriftedLease => {
                presence.drift_return_lease_for_test(part_id);
            }
            tests::ReturnPreflightFault::WorkerCapacity => {
                presence.exhaust_return_workers_for_test(credential);
            }
            tests::ReturnPreflightFault::SessionOverflow => {
                presence.exhaust_return_session_for_test();
            }
            tests::ReturnPreflightFault::SignOverflow => {
                presence.exhaust_return_sign_for_test();
            }
        }
    }

    #[cfg(test)]
    fn atomic_return_state_for_test(
        &self,
    ) -> (
        conduit_body::AdmissionManager,
        (conduit_body::HostPresenceTable, usize, u64, u64),
        (usize, u64),
    ) {
        (
            self.manager.clone().expect("test spawn manager exists"),
            self.presence
                .as_ref()
                .expect("test browser presence exists")
                .atomic_state_for_test(),
            self.returns.atomic_state_for_test(),
        )
    }
}

fn planned_webrtc_bindings(
    plan: &conduit_core::Plan,
) -> Result<Vec<conduit_wire::SessionBinding>, String> {
    let mut bindings = Vec::with_capacity(conduit_body::MAX_BODY_PARTS);
    for source in &plan.fragments {
        for connection in &source.connections {
            let Some(line) = &connection.selected_line else {
                continue;
            };
            if line.binding.base
                != conduit_core::BaseImplementationId::from("conduit.base/webrtc-data-channel@1")
            {
                continue;
            }
            if source.host_id != line.binding.source.host_id
                || source.boot_id != line.binding.source.boot_id
            {
                return Err("planned WebRTC source fragment does not match selected Line".into());
            }
            let mut sinks = plan.fragments.iter().filter(|fragment| {
                fragment.host_id == line.binding.sink.host_id
                    && fragment.boot_id == line.binding.sink.boot_id
            });
            let sink = sinks
                .next()
                .ok_or("planned WebRTC sink fragment is absent")?;
            if sinks.next().is_some() {
                return Err("planned WebRTC sink fragment is ambiguous".into());
            }
            if bindings.len() == conduit_std_host::browser_admission::MAX_WEBRTC_NEGOTIATIONS {
                return Err("planned WebRTC grant capacity exhausted".into());
            }
            bindings.push(
                conduit_wire::SessionBinding::from_planned_connection(
                    plan.plan_id.clone(),
                    source.fragment_id.clone(),
                    sink.fragment_id.clone(),
                    connection,
                )
                .map_err(|error| format!("derive planned browser WebRTC binding: {error:?}"))?,
            );
        }
    }
    Ok(bindings)
}

fn receive_spawn(listener: BrowserAdmissionListener) -> Result<SpawnArrival, String> {
    let mut socket = listener.accept().map_err(debug("accept"))?;
    let advertisement = match socket.receive().map_err(debug("advertisement"))? {
        BrowserAdmissionIngress::Advertise { advertisement, .. } => advertisement,
        _ => return Err("browser spawn did not begin with an advertisement".into()),
    };
    let proof = match socket.receive().map_err(debug("proof"))? {
        BrowserAdmissionIngress::SpawnProof {
            invitation_id,
            body_id,
            host_id,
            boot_id,
            nonce,
            signature,
            ..
        } => SpawnAdmissionProof {
            invitation_id,
            body_id,
            host_id,
            boot_id,
            nonce: nonce
                .try_into()
                .map_err(|_| "invalid browser spawn nonce")?,
            signature: signature
                .try_into()
                .map_err(|_| "invalid browser spawn signature")?,
        },
        _ => return Err("browser spawn did not provide an invitation proof".into()),
    };
    Ok(SpawnArrival {
        listener,
        socket,
        advertisement,
        proof,
    })
}

fn now_millis() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock before epoch: {error}"))?
        .as_millis();
    u64::try_from(millis).map_err(|_| "system clock exceeds Body admission range".into())
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                vec![char::from(byte)]
            } else {
                format!("%{byte:02X}").chars().collect()
            }
        })
        .collect()
}

fn spawn_target(page_url: &str, chat_url: &str, body_url: &str, envelope_hex: &str) -> String {
    format!(
        "{page_url}?ws={}#body={}&spawn_hex={envelope_hex}",
        percent_encode(chat_url),
        percent_encode(body_url),
    )
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}

#[cfg(test)]
#[path = "browser_parts_tests.rs"]
mod tests;
