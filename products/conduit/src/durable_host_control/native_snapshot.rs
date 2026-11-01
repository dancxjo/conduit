//! Existing exact-Host authorization, without starting a loopback browser worker.
use super::{DurableHostRuntime, HostSource, Request, Response, PROTOCOL};
use crate::durable_host::owner::BrowserWindowAuthorization;
use std::path::Path;
impl DurableHostRuntime {
    pub(super) fn authorize_native_snapshot_window(
        &mut self,
        expected_host: &str,
        maximum_millis: u64,
    ) -> Result<BrowserWindowAuthorization, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        // None forbids creating a fresh Part. Existing owner policy retains
        // the Lulled, exact Host, finite lifetime and single-window gates.
        owner.browser_authorize_window(expected_host, None, maximum_millis)
    }
}
#[cfg(unix)]
pub(crate) fn start(
    state: &Path,
    expected_host: &str,
    maximum_millis: u64,
    selected: &conduit_protected_line::SessionBinding,
    route: &conduit_body::RendezvousCandidate,
) -> Result<VerifiedSelection, String> {
    let before = super::current(state)?.advertisement;
    let response = super::body::call(
        state,
        Request::BodyNativeSnapshotStart {
            protocol: PROTOCOL,
            token: super::body::token(state)?,
            expected_host_id: expected_host.into(),
            maximum_millis,
        },
    )?;
    let Response::BodyNativeSnapshotWindow {
        protocol: PROTOCOL,
        authorization,
    } = response
    else {
        return Err("owner refused native snapshot window".into());
    };
    let after = super::current(state)?.advertisement;
    if before.host_id != after.host_id
        || before.boot_id != after.boot_id
        || authorization.owner_host_id != after.host_id
        || authorization.owner_boot_id != after.boot_id
        || authorization.maximum_millis != maximum_millis
    {
        let _ = super::browser::cancel(state, &authorization.window_id);
        return Err("native snapshot window owner Boot or limits changed".into());
    }
    if selected.responder.host_id != after.host_id.as_str()
        || selected.responder.boot_id != after.boot_id.as_str()
        || selected.initiator.host_id != expected_host
        || selected.candidate_binding != route.candidate_id
    {
        let _ = super::browser::cancel(state, &authorization.window_id);
        return Err("native snapshot selected endpoints or candidate mismatch".into());
    }
    Ok(VerifiedSelection {
        authorization,
        selected: selected.clone(),
        route: route.clone(),
    })
}

impl DurableHostRuntime {
    pub(super) fn native_snapshot_face(
        &self,
        window: &str,
        binding: &conduit_core::LinkBindingId,
        request: &conduit_face::OwnerFaceSnapshotRequest,
    ) -> Result<conduit_face::OwnerFaceSnapshotResponse, String> {
        let HostSource::Body { owner, .. } = &self.host else {
            return Err("installed Host does not own a Body".into());
        };
        let face = owner.native_snapshot_in_window(window, binding, request)?;
        let response = conduit_face::OwnerFaceSnapshotResponse::Snapshot {
            schema: conduit_face::OWNER_FACE_RESPONSE_SCHEMA.into(),
            face: Box::new(face),
            interactions_admitted: false,
            route: None,
        };
        if serde_json::to_vec(&response)
            .map_err(|_| "native snapshot response encoding")?
            .len()
            > conduit_face::MAX_OWNER_FACE_RESPONSE_BYTES
        {
            return Err("native snapshot exceeds canonical response profile".into());
        }
        Ok(response)
    }
}
#[cfg(unix)]
pub(crate) fn face(
    state: &Path,
    window: &str,
    binding: conduit_core::LinkBindingId,
    request: conduit_face::OwnerFaceSnapshotRequest,
) -> Result<conduit_face::OwnerFaceSnapshotResponse, String> {
    match super::body::call(
        state,
        Request::BodyNativeSnapshotFace {
            protocol: PROTOCOL,
            token: super::body::token(state)?,
            window_id: window.into(),
            binding,
            request,
        },
    )? {
        Response::Face {
            protocol: PROTOCOL,
            response,
        } => Ok(*response),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong native snapshot Face response".into()),
    }
}

/// Local capability issued only through authenticated actual owner control.
/// Deliberately not serializable: private handoff bytes cannot recreate it.
pub(crate) struct VerifiedSelection {
    authorization: BrowserWindowAuthorization,
    selected: conduit_protected_line::SessionBinding,
    route: conduit_body::RendezvousCandidate,
}
impl VerifiedSelection {
    pub(crate) fn authorization(&self) -> &BrowserWindowAuthorization {
        &self.authorization
    }
    pub(crate) fn binding(&self) -> &conduit_protected_line::SessionBinding {
        &self.selected
    }
    pub(crate) fn route(&self) -> &conduit_body::RendezvousCandidate {
        &self.route
    }
}
