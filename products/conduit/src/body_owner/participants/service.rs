//! Service-owned browser authorization. The carrier worker never owns membership
//! truth: every challenge and completion is applied on the serialized owner actor.
mod admission;
mod route;
#[cfg(test)]
mod tests;
use super::{
    admission::remaining, debug, nonce, now, signal, Owner, MAX_BROWSER_PRESENCE_MILLIS, PROTOCOL,
};
use conduit_body::{
    disclose_host_offer, AdmissionChallenge, AdmissionManager, AmbientAdmissionProof,
    BodyBiographyEvidence, BodyId, BodyState, CandidateInventory, CandidateObservation,
    DiscoveryProofId, HostOfferProjection, MembershipCredential, OfferDisclosureRefusal,
    OfferDisclosureRequest, OfferDisclosureStage, PartReturnChallenge, PartReturnProof,
    RemoteProofClass,
};
use conduit_core::{AuthorityGrantId, HostAdvertisement, HostId, LineOffer, LinkBindingId};
use conduit_presentation::RemoteOwnerMaskRouteSeal;
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In, MAX_BROWSER_ADMISSION_FRAME_BYTES,
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
    pub(crate) presence_maximum_millis: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct BrowserAdmittedSnapshot {
    pub(crate) credential: MembershipCredential,
    pub(crate) biography: Box<BodyBiographyEvidence>,
    pub(crate) offer: Box<HostOfferProjection>,
    pub(crate) owner_advertisement: Option<Box<HostAdvertisement>>,
    pub(crate) browser_advertisement: Option<Box<HostAdvertisement>>,
    pub(crate) line_authorization: Option<BrowserLineAuthorization>,
}

/// Owner-issued authority for only the two directions of this admitted carrier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BrowserLineAuthorization {
    pub(crate) window_id: String,
    pub(crate) carrier_binding: LinkBindingId,
    pub(crate) credential_id: String,
    pub(crate) face_grant_id: AuthorityGrantId,
    pub(crate) return_grant_id: AuthorityGrantId,
}

impl BrowserLineAuthorization {
    pub(super) fn issue(
        window_id: &str,
        binding: &LinkBindingId,
        credential: &MembershipCredential,
    ) -> Self {
        Self {
            window_id: window_id.into(),
            carrier_binding: binding.clone(),
            credential_id: credential.credential_id.as_str().into(),
            face_grant_id: AuthorityGrantId::from(format!(
                "grant/{window_id}/{}/face",
                binding.as_str()
            )),
            return_grant_id: AuthorityGrantId::from(format!(
                "grant/{window_id}/{}/return",
                binding.as_str()
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BrowserCarrierLineEvidence {
    pub(crate) authorization: BrowserLineAuthorization,
    pub(crate) face: LineOffer,
    pub(crate) returned: LineOffer,
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
            owner_advertisement: None,
            browser_advertisement: None,
            line_authorization: None,
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
    Pending(Box<Pending>),
    Active {
        presence_deadline: Instant,
        credential: MembershipCredential,
        observation: Box<CandidateObservation>,
        line_authorization: Box<BrowserLineAuthorization>,
        line_evidence: Option<Box<BrowserCarrierLineEvidence>>,
        route: Option<Box<RemoteOwnerMaskRouteSeal>>,
        acknowledged_show: Option<Box<conduit_presentation::MaskShow>>,
    },
}

pub(super) fn planning_offer_response(
    state_dir: Option<&Path>,
    window_id: Option<&str>,
    credential: &MembershipCredential,
    request: OfferDisclosureRequest,
) -> Result<Out, String> {
    #[cfg(unix)]
    let result = state_dir
        .zip(window_id)
        .ok_or_else(|| "owner-unavailable".to_string())
        .and_then(|(dir, window_id)| {
            crate::durable_host_control::browser::planning_offer(
                dir,
                window_id,
                credential.clone(),
                request,
            )
        });
    #[cfg(not(unix))]
    let result: Result<HostOfferProjection, String> = {
        let _ = (state_dir, window_id, credential, request);
        Err("owner-unavailable".into())
    };
    match result {
        Ok(offer) => {
            let frame = Out::OfferEvidence {
                protocol: PROTOCOL,
                evidence: Box::new(offer),
            };
            if serde_json::to_vec(&frame)
                .map_err(|error| format!("encode planning offer: {error}"))?
                .len()
                > MAX_BROWSER_ADMISSION_FRAME_BYTES
            {
                Ok(Out::Refused {
                    protocol: PROTOCOL,
                    code: "offer-frame-pressure".into(),
                })
            } else {
                Ok(frame)
            }
        }
        Err(error) => Ok(Out::Refused {
            protocol: PROTOCOL,
            code: planning_offer_refusal(&error).into(),
        }),
    }
}

fn planning_offer_refusal(error: &str) -> &'static str {
    match error {
        "unknown-capability" => "unknown-capability",
        "unknown-resource" => "unknown-resource",
        "invalid-offer-request" => "invalid-offer-request",
        "credential-mismatch" => "credential-mismatch",
        "part-unavailable" => "part-unavailable",
        "window-not-active" => "window-not-active",
        "owner-unavailable" => "owner-unavailable",
        "browser-mask-offer-mismatch" => "browser-mask-offer-mismatch",
        _ => "offer-unavailable",
    }
}

struct Pending {
    binding: LinkBindingId,
    observation: CandidateObservation,
    kind: PendingKind,
}

enum PendingKind {
    Ambient {
        challenge: AdmissionChallenge,
        observed_candidates: CandidateInventory,
        verifying_key: [u8; 32],
    },
    Returning {
        challenge: PartReturnChallenge,
        advertisement: HostAdvertisement,
    },
}

impl BrowserWindow {
    fn check(&self, id: &str) -> Result<u64, String> {
        if self.id != id {
            return Err("browser admission window identity differs".into());
        }
        let deadline = match &self.state {
            WindowState::Active {
                presence_deadline, ..
            } => *presence_deadline,
            WindowState::Ready | WindowState::Pending(_) => self.deadline,
        };
        remaining(deadline)?;
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
            presence_maximum_millis: MAX_BROWSER_PRESENCE_MILLIS,
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
            if matches!(window.state, WindowState::Pending(_)) {
                window.state = WindowState::Ready;
            }
            Ok(())
        })();
        self.pending_browser = Some(window);
        result
    }

    /// End a worker's authorization after its bounded carrier loop. If the
    /// worker died after proof but before leave, fence only its retained exact
    /// Host, Boot, and Part before discarding the authorization.
    pub(crate) fn browser_cancel_window(
        &mut self,
        root: &Path,
        window_id: &str,
    ) -> Result<(), String> {
        let window = self
            .pending_browser
            .as_ref()
            .ok_or("no browser admission window")?;
        if window.id != window_id {
            return Err("browser admission window identity differs".into());
        }
        if let WindowState::Active { credential, .. } = &window.state {
            let credential = credential.clone();
            self.browser_leave(root, window_id, &credential)?;
        }
        self.pending_browser = None;
        Ok(())
    }

    /// Disclose only the requested detail from the observation authenticated
    /// for this current browser carrier. It remains self-reported offer truth;
    /// planning and prepare must separately admit an actual Line and Back.
    pub(crate) fn browser_planning_offer(
        &self,
        window_id: &str,
        credential: &MembershipCredential,
        request: &OfferDisclosureRequest,
    ) -> Result<HostOfferProjection, String> {
        let window = self.pending_browser.as_ref().ok_or("window-not-active")?;
        window.check(window_id).map_err(|_| "window-not-active")?;
        let WindowState::Active {
            credential: active,
            observation,
            ..
        } = &window.state
        else {
            return Err("window-not-active".into());
        };
        if active != credential
            || observation.advertisement.host_id != credential.host_id
            || observation.advertisement.boot_id != credential.boot_id
        {
            return Err("credential-mismatch".into());
        }
        if request.stage != OfferDisclosureStage::Planning
            || (request.capability_ids.is_empty() && request.resource_pool_ids.is_empty())
        {
            return Err("invalid-offer-request".into());
        }
        let current = self.session.evidence().membership.parts.iter().any(|part| {
            part.part_id == credential.part_id
                && part.current.as_ref().is_some_and(|host| {
                    host.host_id == credential.host_id && host.boot_id == credential.boot_id
                })
        });
        let admitted = self.admissions.as_ref().is_some_and(|manager| {
            manager
                .receipts
                .iter()
                .rev()
                .find(|receipt| receipt.credential.part_id == credential.part_id)
                .is_some_and(|receipt| receipt.credential == *credential)
        });
        if !current || !admitted || self.session.evidence().body_id != credential.body_id {
            return Err("part-unavailable".into());
        }
        if request
            .capability_ids
            .contains(&conduit_browser_mask_offer::offer().capability_id)
        {
            // Disclosure remains self-reported. Check that this current
            // observation admits the reviewed ordinary browser Mask Plan.
            // This alone does not admit a cross-Host presentation Line.
            if !observation
                .advertisement
                .capabilities
                .contains(&conduit_browser_mask_offer::offer())
                || conduit_browser_mask_offer::planned_mask(
                    &observation.advertisement,
                    conduit_browser_mask_offer::MASK_SOURCE,
                    "browser-graphical",
                )
                .is_err()
            {
                return Err("browser-mask-offer-mismatch".into());
            }
        }
        disclose_host_offer(observation, RemoteProofClass::SelfReported, request).map_err(|error| {
            match error {
                OfferDisclosureRefusal::UnknownCapability => "unknown-capability",
                OfferDisclosureRefusal::UnknownResource => "unknown-resource",
                _ => "invalid-offer-request",
            }
            .into()
        })
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
            let WindowState::Active {
                credential: active, ..
            } = &window.state
            else {
                return Err("browser admission window has no active carrier".into());
            };
            if active != credential {
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
