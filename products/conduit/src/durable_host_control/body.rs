//! Authenticated local control of the one retained Body on a service Boot.
//! The service holds `body-owner.lock` for its lifetime, so these operations
//! are the only live writer of its biography and invitation authority.

use super::DurableHostRuntime;
#[cfg(unix)]
use super::{read_frame, read_secret, write_frame};
#[cfg(any(unix, test))]
use super::{Request, Response, PROTOCOL};
use conduit_body::{
    PortableAdmissionReceipt, PortableInvitation, PortableSpawnAdmissionRequest,
    RendezvousCandidate,
};
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
        | Request::BodyAdmit { token, .. } => token.fill(0),
        _ => unreachable!("Body control client only sends Body requests"),
    }
    sent?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish Body owner control request: {error}"))?;
    read_frame(&mut stream)
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
