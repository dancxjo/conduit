//! Authenticated local control of the one retained Body on a service Boot.
//! The service holds `body-owner.lock` for its lifetime, so these operations
//! are the only live writer of its biography and invitation authority.

#[cfg(unix)]
use super::read_secret;
use super::DurableHostRuntime;
#[cfg(any(unix, test))]
use super::{Request, Response, PROTOCOL};
use conduit_body::{
    BodyBiographyEvidence, HostOfferProjection, MembershipCredential, OfferDisclosureRequest,
    PortableAdmissionReceipt, PortableInvitation, PortableSpawnAdmissionRequest,
    RendezvousCandidate,
};
use conduit_core::LinkBindingId;
use conduit_presentation::{
    FaceInteraction, MaskShow, OwnerFaceSnapshotRequest, OwnerFaceSnapshotResponse, Presentation,
    MAX_OWNER_FACE_RESPONSE_BYTES, OWNER_FACE_RESPONSE_SCHEMA,
};
use conduit_std_host::browser_admission::{BrowserAdmissionEgress, BrowserAdmissionIngress};
use conduit_std_host::StdHost;
use std::{
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

#[path = "body/clock_action.rs"]
mod clock_action;
#[cfg(unix)]
#[path = "body/control_client.rs"]
mod control_client;
#[cfg(unix)]
pub(super) use control_client::call;

pub(super) enum HostSource {
    Bare(Box<StdHost>),
    /// Occupied only while the same Host moves into a journaled Body owner.
    Transitioning,
    Body {
        owner: Box<crate::durable_host::owner::Owner>,
        root: PathBuf,
        running: Option<crate::durable_host::owner::RunWorker>,
    },
}

impl HostSource {
    pub(super) fn advertisement(&self) -> &conduit_core::HostAdvertisement {
        match self {
            Self::Bare(host) => host.advertisement(),
            Self::Transitioning => unreachable!("Host transition is synchronous"),
            Self::Body { owner, .. } => owner.host.advertisement(),
        }
    }

    pub(super) fn body_is_running(&self) -> bool {
        matches!(
            self,
            Self::Body {
                running: Some(_),
                ..
            }
        )
    }
}

impl Deref for HostSource {
    type Target = StdHost;

    fn deref(&self) -> &StdHost {
        match self {
            Self::Bare(host) => host,
            Self::Transitioning => unreachable!("Host transition is synchronous"),
            Self::Body { owner, .. } => owner.host.current(),
        }
    }
}

impl DerefMut for HostSource {
    fn deref_mut(&mut self) -> &mut StdHost {
        match self {
            Self::Bare(host) => host,
            Self::Transitioning => unreachable!("Host transition is synchronous"),
            Self::Body { owner, .. } => owner.host.current_mut(),
        }
    }
}

impl DurableHostRuntime {
    pub(super) fn start_browser_window(
        &mut self,
        expected_host_id: &str,
        new_host_verifying_key: Option<[u8; 32]>,
        maximum_millis: u64,
    ) -> Result<
        (
            crate::durable_host::owner::BrowserWindowAuthorization,
            String,
        ),
        String,
    > {
        let HostSource::Body { owner, root, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        let authorization = owner.browser_authorize_window(
            expected_host_id,
            new_host_verifying_key,
            maximum_millis,
        )?;
        #[cfg(unix)]
        {
            match super::browser::spawn_window(root, authorization.clone()) {
                Ok(url) => Ok((authorization, url)),
                Err(error) => {
                    let _ = owner.browser_cancel_window(root, &authorization.window_id);
                    Err(error)
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = root;
            let _ = owner.browser_cancel_window(root, &authorization.window_id);
            Err("browser admission worker requires local Unix control".into())
        }
    }

    pub(super) fn browser_begin(
        &mut self,
        window_id: &str,
        binding: &LinkBindingId,
        frame: BrowserAdmissionIngress,
        encoded_bytes: u32,
    ) -> Result<BrowserAdmissionEgress, String> {
        match &mut self.host {
            HostSource::Body { owner, .. } => {
                owner.browser_begin(window_id, binding, frame, encoded_bytes)
            }
            HostSource::Bare(_) | HostSource::Transitioning => {
                Err("installed Host does not own a live Body session".into())
            }
        }
    }

    pub(super) fn browser_complete(
        &mut self,
        window_id: &str,
        frame: BrowserAdmissionIngress,
    ) -> Result<crate::durable_host::owner::BrowserAdmittedSnapshot, String> {
        match &mut self.host {
            HostSource::Body { owner, root, .. } => owner.browser_complete(root, window_id, frame),
            HostSource::Bare(_) | HostSource::Transitioning => {
                Err("installed Host does not own a live Body session".into())
            }
        }
    }

    pub(super) fn browser_planning_offer(
        &self,
        window_id: &str,
        credential: &MembershipCredential,
        disclosure: &OfferDisclosureRequest,
    ) -> Result<HostOfferProjection, String> {
        match &self.host {
            HostSource::Body { owner, .. } => {
                owner.browser_planning_offer(window_id, credential, disclosure)
            }
            HostSource::Bare(_) | HostSource::Transitioning => {
                Err("installed Host does not own a live Body session".into())
            }
        }
    }

    pub(super) fn browser_abort(&mut self, window_id: &str) -> Result<(), String> {
        match &mut self.host {
            HostSource::Body { owner, .. } => owner.browser_abort(window_id),
            HostSource::Bare(_) | HostSource::Transitioning => {
                Err("installed Host does not own a live Body session".into())
            }
        }
    }

    pub(super) fn browser_cancel_window(&mut self, window_id: &str) -> Result<(), String> {
        match &mut self.host {
            HostSource::Body { owner, root, .. } => owner.browser_cancel_window(root, window_id),
            HostSource::Bare(_) | HostSource::Transitioning => {
                Err("installed Host does not own a live Body session".into())
            }
        }
    }

    pub(super) fn browser_leave(
        &mut self,
        window_id: &str,
        credential: &MembershipCredential,
    ) -> Result<BodyBiographyEvidence, String> {
        match &mut self.host {
            HostSource::Body { owner, root, .. } => {
                owner.browser_leave(root, window_id, credential)
            }
            HostSource::Bare(_) | HostSource::Transitioning => {
                Err("installed Host does not own a live Body session".into())
            }
        }
    }

    pub(crate) fn with_owned_body(self, root: &Path) -> Result<Self, String> {
        let Self {
            target_id,
            image_content_digest,
            host,
            birth,
            birth_root,
            remote_fragment,
            pool_member,
            cancellation_signal,
            next_observation_sequence,
            #[cfg(unix)]
            terminal_route,
        } = self;
        let HostSource::Bare(host) = host else {
            return Err("durable Host already owns a Body session".into());
        };
        let owner = crate::durable_host::owner::resume_service(*host, root)?;
        Ok(Self {
            target_id,
            image_content_digest,
            host: HostSource::Body {
                owner: Box::new(owner),
                root: root.to_path_buf(),
                running: None,
            },
            birth,
            birth_root,
            remote_fragment,
            pool_member,
            cancellation_signal,
            next_observation_sequence,
            #[cfg(unix)]
            terminal_route,
        })
    }

    pub(super) fn owned_body_truth(&self) -> Result<serde_json::Value, String> {
        match &self.host {
            HostSource::Body { owner, .. } => {
                let mut truth = owner.truth();
                // The one-shot owner route proves admission, not a continuing
                // carrier lease. Retained `membership.current` is not a live
                // reachability observation after that route closes.
                truth["remote_carrier_availability"] = serde_json::json!("unobserved");
                Ok(truth)
            }
            HostSource::Bare(_) | HostSource::Transitioning => {
                Err("installed Host does not own a live Body session".into())
            }
        }
    }

    pub(super) fn owned_body_face(
        &self,
        request: &OwnerFaceSnapshotRequest,
    ) -> Result<OwnerFaceSnapshotResponse, String> {
        let HostSource::Body { owner, .. } = &self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        let presentation = owner.face_snapshot(request)?;
        let response = if request.last_seen_revision == Some(presentation.revision)
            && request.last_seen_identity.as_ref() == Some(&presentation.identity)
        {
            OwnerFaceSnapshotResponse::Unchanged {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                revision: presentation.revision,
                identity: presentation.identity,
            }
        } else {
            OwnerFaceSnapshotResponse::Snapshot {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                presentation: Box::new(presentation),
                interactions_admitted: false,
            }
        };
        let encoded = serde_json::to_vec(&response)
            .map_err(|error| format!("encode owner Face snapshot: {error}"))?;
        if encoded.len() > MAX_OWNER_FACE_RESPONSE_BYTES {
            return Err("face-frame-pressure".into());
        }
        Ok(response)
    }

    pub(super) fn owned_body_local_face(
        &self,
    ) -> Result<(Presentation, conduit_core::HostAdvertisement), String> {
        let HostSource::Body { owner, .. } = &self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        let presentation = owner.local_face_snapshot()?;
        if serde_json::to_vec(&presentation)
            .map_err(|error| format!("encode local owner Face: {error}"))?
            .len()
            > MAX_OWNER_FACE_RESPONSE_BYTES
        {
            return Err("face-frame-pressure".into());
        }
        Ok((presentation, owner.host.advertisement().clone()))
    }

    pub(super) fn issue_owned_invitation(
        &mut self,
        ttl_seconds: u64,
        candidates: Option<Vec<RendezvousCandidate>>,
    ) -> Result<PortableInvitation, String> {
        match &mut self.host {
            HostSource::Body { owner, root, .. } => {
                owner.issue_invitation(root, ttl_seconds, candidates)
            }
            HostSource::Bare(_) | HostSource::Transitioning => {
                Err("installed Host does not own a live Body session".into())
            }
        }
    }

    pub(super) fn admit_owned_request(
        &mut self,
        request: PortableSpawnAdmissionRequest,
    ) -> Result<PortableAdmissionReceipt, String> {
        match &mut self.host {
            HostSource::Body { owner, root, .. } => {
                // Possession of the retained single-use invitation is the routed
                // authorization. The ordinary signed request must still name
                // its exact candidate Host and current Boot.
                let expected = request.host_advertisement.host_id.as_str().to_owned();
                owner.admit_invited(root, request, &expected)
            }
            HostSource::Bare(_) | HostSource::Transitioning => {
                Err("installed Host does not own a live Body session".into())
            }
        }
    }
}

#[cfg(unix)]
pub(crate) fn start_browser_window(
    state_dir: &Path,
    expected_host_id: &str,
    new_host_verifying_key: Option<[u8; 32]>,
    maximum_millis: u64,
) -> Result<(), String> {
    match call(
        state_dir,
        Request::BodyBrowserStart {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            expected_host_id: expected_host_id.into(),
            new_host_verifying_key,
            maximum_millis,
        },
    )? {
        Response::BodyBrowserWindow {
            protocol: PROTOCOL,
            window_id,
            url,
            body_id,
            maximum_millis,
        } => {
            println!(
                "{}",
                serde_json::json!({
                    "schema":"conduit.body/browser-admission-window@1",
                    "window_id":window_id,
                    "url":url,
                    "body_id":body_id,
                    "expected_host_id":expected_host_id,
                    "maximum_millis":maximum_millis,
                    "remote_execution":false,
                })
            );
            Ok(())
        }
        Response::Refused { code, .. } => Err(format!("Body owner refused browser window: {code}")),
        _ => Err("Body owner returned the wrong browser-window response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn start_browser_window(
    _state_dir: &Path,
    _expected_host_id: &str,
    _new_host_verifying_key: Option<[u8; 32]>,
    _maximum_millis: u64,
) -> Result<(), String> {
    Err("browser admission window requires local Unix control".into())
}

#[cfg(unix)]
pub(super) fn token(state_dir: &Path) -> Result<Vec<u8>, String> {
    let mut secret = read_secret(&state_dir.join("control.token"))?;
    let value = secret.to_vec();
    secret.fill(0);
    Ok(value)
}

#[cfg(unix)]
pub(crate) fn inspect_owned_body(state_dir: &Path) -> Result<serde_json::Value, String> {
    match call(
        state_dir,
        Request::BodyInspect {
            protocol: PROTOCOL,
            token: token(state_dir)?,
        },
    )? {
        Response::BodyTruth {
            protocol: PROTOCOL,
            truth,
        } => Ok(truth),
        Response::Refused { code, .. } => Err(format!("Body owner refused inspect: {code}")),
        _ => Err("Body owner returned the wrong inspect response".into()),
    }
}

#[cfg(unix)]
pub(crate) fn issue_owned_invitation(
    state_dir: &Path,
    ttl_seconds: u64,
    candidates: Option<Vec<RendezvousCandidate>>,
) -> Result<PortableInvitation, String> {
    match call(
        state_dir,
        Request::BodyInvite {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            ttl_seconds,
            candidates,
        },
    )? {
        Response::BodyInvitation {
            protocol: PROTOCOL,
            invitation,
        } => Ok(*invitation),
        Response::Refused { code, .. } => Err(format!("Body owner refused invitation: {code}")),
        _ => Err("Body owner returned the wrong invitation response".into()),
    }
}

#[cfg(unix)]
pub(crate) fn admit_owned_request(
    state_dir: &Path,
    request: PortableSpawnAdmissionRequest,
) -> Result<PortableAdmissionReceipt, String> {
    match call(
        state_dir,
        Request::BodyAdmit {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            request: Box::new(request),
        },
    )? {
        Response::BodyAdmitted {
            protocol: PROTOCOL,
            receipt,
        } => Ok(*receipt),
        Response::Refused { code, .. } => Err(format!("Body owner refused admission: {code}")),
        _ => Err("Body owner returned the wrong admission response".into()),
    }
}

#[cfg(unix)]
pub(crate) fn face_snapshot(
    state_dir: &Path,
    request: OwnerFaceSnapshotRequest,
) -> Result<OwnerFaceSnapshotResponse, String> {
    match call(
        state_dir,
        Request::BodyFace {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            request,
        },
    )? {
        Response::BodyFace {
            protocol: PROTOCOL,
            response,
        } => Ok(*response),
        Response::Refused { code, .. } => Err(code),
        _ => Err("Body owner returned the wrong Face response".into()),
    }
}

#[cfg(unix)]
pub(crate) fn local_face_snapshot(
    state_dir: &Path,
) -> Result<(Presentation, conduit_core::HostAdvertisement), String> {
    match call(
        state_dir,
        Request::BodyLocalFace {
            protocol: PROTOCOL,
            token: token(state_dir)?,
        },
    )? {
        Response::BodyLocalFace {
            protocol: PROTOCOL,
            presentation,
            advertisement,
        } => Ok((*presentation, advertisement)),
        Response::Refused { code, .. } => Err(format!("Body owner refused local Face: {code}")),
        _ => Err("Body owner returned the wrong local Face response".into()),
    }
}

#[cfg(unix)]
pub(crate) fn submit_local_face_interaction(
    state_dir: &Path,
    show: MaskShow,
    interaction: FaceInteraction,
) -> Result<serde_json::Value, String> {
    submit_local_face_interaction_with_expiry(state_dir, show, interaction, None)
}

#[cfg(unix)]
pub(crate) fn submit_attached_terminal_interaction(
    state_dir: &Path,
    route_plan_id: conduit_core::PlanId,
    show: MaskShow,
    interaction: FaceInteraction,
) -> Result<serde_json::Value, String> {
    match call(
        state_dir,
        Request::BodyAttachedTerminalInteraction {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            route_plan_id,
            show: Box::new(show),
            interaction,
        },
    )? {
        Response::BodyInteraction {
            protocol: PROTOCOL,
            result,
        } => Ok(*result),
        Response::Refused { code, .. } => {
            Err(format!("Body owner refused terminal action: {code}"))
        }
        _ => Err("Body owner returned the wrong terminal action response".into()),
    }
}

#[cfg(unix)]
#[allow(dead_code)] // Native return consumes this exact expiry-bearing entrance after its route lands.
pub(crate) fn submit_local_face_interaction_until(
    state_dir: &Path,
    show: MaskShow,
    interaction: FaceInteraction,
    not_after_millis: u64,
) -> Result<serde_json::Value, String> {
    submit_local_face_interaction_with_expiry(state_dir, show, interaction, Some(not_after_millis))
}

#[cfg(unix)]
fn submit_local_face_interaction_with_expiry(
    state_dir: &Path,
    show: MaskShow,
    interaction: FaceInteraction,
    not_after_millis: Option<u64>,
) -> Result<serde_json::Value, String> {
    match call(
        state_dir,
        Request::BodyInteraction {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            show: Box::new(show),
            interaction,
            not_after_millis,
        },
    )? {
        Response::BodyInteraction {
            protocol: PROTOCOL,
            result,
        } => Ok(*result),
        Response::Refused { code, .. } => Err(format!("Body owner refused interaction: {code}")),
        _ => Err("Body owner returned the wrong interaction response".into()),
    }
}

#[cfg(unix)]
pub(crate) fn submit_browser_face_interaction(
    state_dir: &Path,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
    interaction: FaceInteraction,
) -> Result<serde_json::Value, String> {
    submit_browser_face_interaction_with_expiry(state_dir, request, show, interaction, None)
}

#[cfg(unix)]
pub(crate) fn submit_browser_face_interaction_until(
    state_dir: &Path,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
    interaction: FaceInteraction,
    not_after_millis: u64,
) -> Result<serde_json::Value, String> {
    submit_browser_face_interaction_with_expiry(
        state_dir,
        request,
        show,
        interaction,
        Some(not_after_millis),
    )
}

#[cfg(unix)]
fn submit_browser_face_interaction_with_expiry(
    state_dir: &Path,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
    interaction: FaceInteraction,
    not_after_millis: Option<u64>,
) -> Result<serde_json::Value, String> {
    match call(
        state_dir,
        Request::BodyBrowserInteraction {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            request,
            show: Box::new(show),
            interaction,
            not_after_millis,
        },
    )? {
        Response::BodyInteraction {
            protocol: PROTOCOL,
            result,
        } => Ok(*result),
        Response::Refused { code, .. } => Err(code),
        _ => Err("Body owner returned the wrong browser interaction response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn submit_browser_face_interaction(
    _state_dir: &Path,
    _request: OwnerFaceSnapshotRequest,
    _show: MaskShow,
    _interaction: FaceInteraction,
) -> Result<serde_json::Value, String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn submit_browser_face_interaction_until(
    _state_dir: &Path,
    _request: OwnerFaceSnapshotRequest,
    _show: MaskShow,
    _interaction: FaceInteraction,
    _not_after_millis: u64,
) -> Result<serde_json::Value, String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn local_face_snapshot(
    _state_dir: &Path,
) -> Result<(Presentation, conduit_core::HostAdvertisement), String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn submit_local_face_interaction(
    _state_dir: &Path,
    _show: MaskShow,
    _interaction: FaceInteraction,
) -> Result<serde_json::Value, String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn submit_local_face_interaction_until(
    _state_dir: &Path,
    _show: MaskShow,
    _interaction: FaceInteraction,
    _not_after_millis: u64,
) -> Result<serde_json::Value, String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn face_snapshot(
    _state_dir: &Path,
    _request: OwnerFaceSnapshotRequest,
) -> Result<OwnerFaceSnapshotResponse, String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn inspect_owned_body(_state_dir: &Path) -> Result<serde_json::Value, String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn issue_owned_invitation(
    _state_dir: &Path,
    _ttl_seconds: u64,
    _candidates: Option<Vec<RendezvousCandidate>>,
) -> Result<PortableInvitation, String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn admit_owned_request(
    _state_dir: &Path,
    _request: PortableSpawnAdmissionRequest,
) -> Result<PortableAdmissionReceipt, String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}

#[cfg(test)]
#[path = "body/tests.rs"]
mod tests;
