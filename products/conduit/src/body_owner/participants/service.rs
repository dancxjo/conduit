//! Service-owned browser authorization. The carrier worker never owns membership
//! truth: every challenge and completion is applied on the serialized owner actor.
mod admission;
use super::{admission::remaining, debug, nonce, now, signal, Owner, PROTOCOL};
use conduit_body::{
    disclose_host_offer, AdmissionId, AdmissionManager, AmbientAdmissionProof,
    BodyBiographyEvidence, BodyId, BodyState, CandidateInventory, CandidateObservation,
    DiscoveryProofId, HostOfferProjection, MembershipCredential, OfferDisclosureRequest,
    OfferDisclosureStage, PartReturnProof, RemoteProofClass,
};
use conduit_core::{HostAdvertisement, HostId, LinkBindingId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In,
};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BrowserWindowAuthorization {
    pub(crate) window_id: String,
    pub(crate) body_id: BodyId,
    pub(crate) maximum_millis: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct BrowserAdmittedSnapshot {
    pub(crate) credential: MembershipCredential,
    pub(crate) biography: Box<BodyBiographyEvidence>,
    pub(crate) offer: Box<HostOfferProjection>,
}

impl BrowserAdmittedSnapshot {
    pub(super) fn from_foreground(
        owner: &Owner,
        credential: MembershipCredential,
        observation: CandidateObservation,
    ) -> Result<Self, String> {
        let offer = disclose_host_offer(
            &observation,
            RemoteProofClass::SelfReported,
            &OfferDisclosureRequest {
                stage: OfferDisclosureStage::AdmittedMembership,
                capability_ids: vec![],
                resource_pool_ids: vec![],
            },
        )
        .map_err(debug)?;
        Ok(Self {
            credential,
            biography: Box::new(owner.session.evidence().clone()),
            offer: Box::new(offer),
        })
    }
}

pub(crate) struct BrowserWindow {
    id: String,
    expected: HostId,
    new_key: Option<[u8; 32]>,
    clock: Instant,
    deadline: Instant,
    state: WindowState,
}

enum WindowState {
    Ready,
    Pending(Pending),
    Active(MembershipCredential),
}

struct Pending {
    admission_id: AdmissionId,
    binding: LinkBindingId,
    observation: CandidateObservation,
    kind: PendingKind,
}

enum PendingKind {
    Ambient(CandidateInventory),
    Returning(HostAdvertisement),
}

impl BrowserWindow {
    fn check(&self, id: &str) -> Result<u64, String> {
        if self.id != id {
            return Err("browser admission window identity differs".into());
        }
        remaining(self.deadline)?;
        Ok(now(self.clock))
    }

    fn expiry(&self, at: u64) -> Result<u64, String> {
        let expires = (self.deadline.duration_since(self.clock).as_millis() as u64)
            .min(at.saturating_add(2000));
        if expires <= at {
            return Err("browser admission authorization window expired".into());
        }
        Ok(expires)
    }
}

impl Owner {
    pub(crate) fn browser_authorize_window(
        &mut self,
        expected_host: &str,
        new_key: Option<[u8; 32]>,
        maximum_millis: u64,
    ) -> Result<BrowserWindowAuthorization, String> {
        if self.session.evidence().body.state != BodyState::Lulled
            || self.session.realization().is_some()
        {
            return Err(
                "browser admission requires a lulled Body without a pending realization".into(),
            );
        }
        if !(1000..=60_000).contains(&maximum_millis)
            || expected_host.is_empty()
            || expected_host.len() > 256
        {
            return Err(
                "browser admission requires an exact Host and a window of 1000..60000 ms".into(),
            );
        }
        let expected = HostId::from(expected_host);
        if expected == self.host.advertisement().host_id {
            return Err("browser participant cannot replace the owner Host".into());
        }
        if self.pending_browser.is_some() {
            return Err("another browser admission window is active".into());
        }
        let known = self.admissions.as_ref().is_some_and(|manager| {
            manager
                .receipts
                .iter()
                .any(|receipt| receipt.credential.host_id == expected)
        });
        if !known && new_key.is_none() {
            return Err("first browser admission requires its exact public verifying key".into());
        }
        let id = format!("browser-window/{}", hex(&nonce()?));
        let clock = Instant::now();
        self.pending_browser = Some(BrowserWindow {
            id: id.clone(),
            expected,
            new_key,
            clock,
            deadline: clock + Duration::from_millis(maximum_millis),
            state: WindowState::Ready,
        });
        Ok(BrowserWindowAuthorization {
            window_id: id,
            body_id: self.session.evidence().body_id.clone(),
            maximum_millis,
        })
    }

    pub(crate) fn browser_abort(&mut self, window_id: &str) -> Result<(), String> {
        let mut window = self
            .pending_browser
            .take()
            .ok_or("no browser admission window")?;
        let result = (|| {
            if window.id != window_id {
                return Err("browser admission window identity differs".into());
            }
            if let WindowState::Pending(pending) = &mut window.state {
                let manager = self
                    .admissions
                    .as_mut()
                    .ok_or("browser admission manager absent")?;
                match &mut pending.kind {
                    PendingKind::Ambient(candidates) => manager
                        .disconnect_ambient(
                            candidates,
                            &pending.admission_id,
                            signal(&pending.binding, "lost"),
                        )
                        .map_err(debug)?,
                    PendingKind::Returning(_) => manager
                        .disconnect_return(&pending.admission_id)
                        .map_err(debug)?,
                }
                window.state = WindowState::Ready;
            }
            Ok(())
        })();
        let finished = result.is_ok()
            && matches!(window.state, WindowState::Ready)
            && Instant::now() >= window.deadline;
        if !finished {
            self.pending_browser = Some(window);
        }
        result
    }

    pub(crate) fn browser_leave(
        &mut self,
        root: &Path,
        window_id: &str,
        credential: &MembershipCredential,
    ) -> Result<BodyBiographyEvidence, String> {
        let mut window = self
            .pending_browser
            .take()
            .ok_or("no browser admission window")?;
        let result = (|| {
            if window.id != window_id {
                return Err("browser admission window identity differs".into());
            }
            let WindowState::Active(active) = &window.state else {
                return Err("browser admission window has no active carrier".into());
            };
            if active != *credential {
                return Err("browser leave credential differs from active carrier".into());
            }
            let current = self
                .session
                .evidence()
                .membership
                .parts
                .iter()
                .find(|part| part.part_id == credential.part_id)
                .and_then(|part| part.current.as_ref())
                .ok_or("browser Part is no longer current")?;
            if current.host_id != credential.host_id || current.boot_id != credential.boot_id {
                return Err("browser leave differs from current Host Boot".into());
            }
            let latest = self.admissions.as_ref().and_then(|manager| {
                manager
                    .receipts
                    .iter()
                    .rev()
                    .find(|receipt| receipt.credential.part_id == credential.part_id)
            });
            if latest.is_none_or(|receipt| receipt.credential != *credential) {
                return Err("browser leave credential is no longer current".into());
            }
            let mut session = self.session.clone();
            let authority = self.host.advertisement();
            session
                .observe_host_lost(
                    &credential.host_id,
                    &credential.boot_id,
                    &authority.host_id,
                    &authority.boot_id,
                )
                .map_err(debug)?;
            let before = std::mem::replace(&mut self.session, session);
            if let Err(error) = self.persist(root) {
                self.session = before;
                return Err(error);
            }
            window.state = WindowState::Ready;
            Ok(self.session.evidence().clone())
        })();
        self.pending_browser = Some(window);
        result
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
