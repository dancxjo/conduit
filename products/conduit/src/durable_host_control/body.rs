//! Authenticated local control of the one retained Body on a service Boot.
//! The service holds `body-owner.lock` for its lifetime, so these operations
//! are the only live writer of its biography and invitation authority.

use super::DurableHostRuntime;
#[cfg(unix)]
use super::{read_frame, read_secret, write_frame};
#[cfg(any(unix, test))]
use super::{Request, Response, PROTOCOL};
use conduit_body::{
    BodyBiographyEvidence, MembershipCredential, PortableAdmissionReceipt, PortableInvitation,
    PortableSpawnAdmissionRequest, RendezvousCandidate,
};
use conduit_core::LinkBindingId;
use conduit_presentation::{
    OwnerFaceSnapshotRequest, OwnerFaceSnapshotResponse, MAX_OWNER_FACE_RESPONSE_BYTES,
    OWNER_FACE_RESPONSE_SCHEMA,
};
use conduit_std_host::browser_admission::{BrowserAdmissionEgress, BrowserAdmissionIngress};
use conduit_std_host::StdHost;
use std::{
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

pub(super) enum HostSource {
    Bare(Box<StdHost>),
    Body {
        owner: Box<crate::durable_host::owner::Owner>,
        root: PathBuf,
    },
}

impl Deref for HostSource {
    type Target = StdHost;

    fn deref(&self) -> &StdHost {
        match self {
            Self::Bare(host) => host,
            Self::Body { owner, .. } => &owner.host,
        }
    }
}

impl DerefMut for HostSource {
    fn deref_mut(&mut self) -> &mut StdHost {
        match self {
            Self::Bare(host) => host,
            Self::Body { owner, .. } => &mut owner.host,
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
        let HostSource::Body { owner, root } = &mut self.host else {
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
            HostSource::Bare(_) => Err("installed Host does not own a live Body session".into()),
        }
    }

    pub(super) fn browser_complete(
        &mut self,
        window_id: &str,
        frame: BrowserAdmissionIngress,
    ) -> Result<crate::durable_host::owner::BrowserAdmittedSnapshot, String> {
        match &mut self.host {
            HostSource::Body { owner, root } => owner.browser_complete(root, window_id, frame),
            HostSource::Bare(_) => Err("installed Host does not own a live Body session".into()),
        }
    }

    pub(super) fn browser_abort(&mut self, window_id: &str) -> Result<(), String> {
        match &mut self.host {
            HostSource::Body { owner, .. } => owner.browser_abort(window_id),
            HostSource::Bare(_) => Err("installed Host does not own a live Body session".into()),
        }
    }

    pub(super) fn browser_cancel_window(&mut self, window_id: &str) -> Result<(), String> {
        match &mut self.host {
            HostSource::Body { owner, root } => owner.browser_cancel_window(root, window_id),
            HostSource::Bare(_) => Err("installed Host does not own a live Body session".into()),
        }
    }

    pub(super) fn browser_leave(
        &mut self,
        window_id: &str,
        credential: &MembershipCredential,
    ) -> Result<BodyBiographyEvidence, String> {
        match &mut self.host {
            HostSource::Body { owner, root } => owner.browser_leave(root, window_id, credential),
            HostSource::Bare(_) => Err("installed Host does not own a live Body session".into()),
        }
    }

    pub(crate) fn with_owned_body(self, root: &Path) -> Result<Self, String> {
        let Self {
            target_id,
            image_content_digest,
            host,
            remote_fragment,
            pool_member,
            cancellation_signal,
            next_observation_sequence,
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
            },
            remote_fragment,
            pool_member,
            cancellation_signal,
            next_observation_sequence,
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
            HostSource::Bare(_) => Err("installed Host does not own a live Body session".into()),
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

    pub(super) fn issue_owned_invitation(
        &mut self,
        ttl_seconds: u64,
        candidates: Option<Vec<RendezvousCandidate>>,
    ) -> Result<PortableInvitation, String> {
        match &mut self.host {
            HostSource::Body { owner, root } => {
                owner.issue_invitation(root, ttl_seconds, candidates)
            }
            HostSource::Bare(_) => Err("installed Host does not own a live Body session".into()),
        }
    }

    pub(super) fn admit_owned_request(
        &mut self,
        request: PortableSpawnAdmissionRequest,
    ) -> Result<PortableAdmissionReceipt, String> {
        match &mut self.host {
            HostSource::Body { owner, root } => {
                // Possession of the retained single-use invitation is the routed
                // authorization. The ordinary signed request must still name
                // its exact candidate Host and current Boot.
                let expected = request.host_advertisement.host_id.as_str().to_owned();
                owner.admit_invited(root, request, &expected)
            }
            HostSource::Bare(_) => Err("installed Host does not own a live Body session".into()),
        }
    }
}

#[cfg(unix)]
fn call(state_dir: &Path, mut request: Request) -> Result<Response, String> {
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to current Body owner service: {error}"))?;
    let sent = write_frame(&mut stream, &request);
    match &mut request {
        Request::BodyInspect { token, .. }
        | Request::BodyInvite { token, .. }
        | Request::BodyAdmit { token, .. }
        | Request::BodyBrowserStart { token, .. }
        | Request::BodyFace { token, .. } => token.fill(0),
        _ => unreachable!("Body control client only sends Body requests"),
    }
    sent?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish Body owner control request: {error}"))?;
    read_frame(&mut stream)
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
fn token(state_dir: &Path) -> Result<Vec<u8>, String> {
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
