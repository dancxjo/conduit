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
}

struct PendingSpawn {
    receiver: Receiver<Result<SpawnArrival, String>>,
    expires_at_millis: u64,
}

pub(super) struct SpawnArrival {
    socket: BrowserAdmissionSocket,
    advertisement: conduit_core::HostAdvertisement,
    proof: SpawnAdmissionProof,
}

impl BrowserPartsCoordinator {
    pub(super) fn new(page_url: String, chat_url: String) -> Self {
        Self {
            page_url,
            chat_url,
            manager: None,
            pending: None,
            ambient: None,
        }
    }

    pub(super) fn start_ambient(&mut self, body_id: &BodyId) -> Result<String, String> {
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
mod tests {
    use super::spawn_target;

    #[test]
    fn spawn_secret_is_fragment_only_and_transport_urls_are_encoded() {
        let target = spawn_target(
            "http://127.0.0.1:8080/index.html",
            "ws://127.0.0.1:9000/chat?line=one",
            "ws://127.0.0.1:9001/admit?body=one",
            "deadbeef",
        );
        let (request, fragment) = target.split_once('#').unwrap();

        assert_eq!(
            request,
            "http://127.0.0.1:8080/index.html?ws=ws%3A%2F%2F127.0.0.1%3A9000%2Fchat%3Fline%3Done"
        );
        assert!(!request.contains("deadbeef"));
        assert_eq!(
            fragment,
            "body=ws%3A%2F%2F127.0.0.1%3A9001%2Fadmit%3Fbody%3Done&spawn_hex=deadbeef"
        );
    }

    #[test]
    fn cancellation_is_explicit_and_idempotently_fail_closed() {
        let mut coordinator = super::BrowserPartsCoordinator::new("page".into(), "chat".into());
        assert!(!coordinator.cancel());
        assert!(!coordinator.is_pending());
    }
}
