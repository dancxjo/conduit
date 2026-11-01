//! One existing-Part Return and read-only Face snapshot on a selected protected Line.
//! This encounter grants no Mask, Show, input, new invitation, or durable client custody.
use crate::durable_host::owner::BrowserAdmissionCompletion;
use crate::durable_host_control::{self as control, browser};
use crate::host_rendezvous::ProtectedDocumentLine;
use conduit_core::{AuthorityGrantId, LinkBindingId};
use conduit_face::{OwnerFaceSnapshotResponse, MAX_OWNER_FACE_RESPONSE_BYTES};
use conduit_protected_line::{
    ProtectedFrameCarrier, Role, SessionBinding, SessionDisposition, SessionLimits,
};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In,
    BROWSER_ADMISSION_PROTOCOL as PROTOCOL, MAX_BROWSER_ADMISSION_FRAME_BYTES,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Immutable diagnostics of this actual retained protected session, not a grant
/// that can be supplied to another carrier or a Mask planner.
pub(crate) struct NativeSnapshotEvidence {
    pub(crate) session: SessionBinding,
    pub(crate) limits: SessionLimits,
    pub(crate) window_id: String,
    pub(crate) credential_id: String,
    pub(crate) face_grant_id: AuthorityGrantId,
}

pub(crate) fn serve<C: ProtectedFrameCarrier>(
    state: &Path,
    verified: &control::native_snapshot::VerifiedSelection,
    mut line: ProtectedDocumentLine<C>,
) -> Result<NativeSnapshotEvidence, String> {
    let authorization = verified.authorization();
    let selected = verified.binding();
    let mut active = None;
    let binding = LinkBindingId::from(format!(
        "binding/native-protected-snapshot/{}",
        selected.line_session_id
    ));
    let result = (|| {
        let evidence = line.evidence();
        if evidence.role != Role::Responder
            || evidence.disposition != SessionDisposition::Open
            || evidence.binding != selected
            || selected.responder.host_id != authorization.owner_host_id.as_str()
            || selected.responder.boot_id != authorization.owner_boot_id.as_str()
            || !(1000..=60_000).contains(&authorization.maximum_millis)
        {
            return Err("native snapshot selected protected responder mismatch".into());
        }
        let limits = evidence.limits;
        line.restrict_deadline(
            Instant::now() + Duration::from_millis(authorization.maximum_millis),
        )?;
        let current = control::current(state)?;
        if current.advertisement.host_id != authorization.owner_host_id
            || current.advertisement.boot_id != authorization.owner_boot_id
        {
            return Err("native snapshot owner Boot changed".into());
        }
        let first = line.receive()?;
        let frame = decode(&first)?;
        let In::ReturnAdvertise {
            protocol: PROTOCOL,
            credential,
            advertisement,
        } = &frame
        else {
            return Err("native snapshot requires existing-Part Return".into());
        };
        if credential.body_id != authorization.body_id
            || credential.host_id != advertisement.host_id
            || selected.initiator.host_id != advertisement.host_id.as_str()
            || selected.initiator.boot_id != advertisement.boot_id.as_str()
        {
            return Err("native snapshot protected initiator or Body mismatch".into());
        }
        let challenge = browser::begin(
            state,
            &authorization.window_id,
            binding.clone(),
            frame,
            first
                .len()
                .try_into()
                .map_err(|_| "native snapshot request size")?,
        )?;
        line.send(&encode(&challenge)?)?;
        let proof = decode(&line.receive()?)?;
        if !matches!(
            proof,
            In::ReturnProof {
                protocol: PROTOCOL,
                ..
            }
        ) {
            return Err("native snapshot requires canonical Return proof".into());
        }
        let snapshot = match browser::complete(state, &authorization.window_id, proof)? {
            BrowserAdmissionCompletion::Active { snapshot } => snapshot,
            BrowserAdmissionCompletion::CustodyOnly { .. } => {
                return Err("custody-only completion cannot open snapshot encounter".into())
            }
        };
        active = Some(snapshot.credential.clone());
        let grant = snapshot
            .line_authorization
            .as_ref()
            .ok_or("native snapshot lacks owner carrier authorization")?;
        if grant.window_id != authorization.window_id
            || grant.carrier_binding != binding
            || grant.credential_id != snapshot.credential.credential_id.as_str()
            || snapshot.credential.body_id != authorization.body_id
            || snapshot.credential.host_id.as_str() != selected.initiator.host_id
            || snapshot.credential.boot_id.as_str() != selected.initiator.boot_id
        {
            return Err("native snapshot owner carrier authorization mismatch".into());
        }
        line.send(&encode(&Out::Admitted {
            protocol: PROTOCOL,
            credential: snapshot.credential.clone(),
            signed_membership_receipt: None,
        })?)?;
        let In::FaceSnapshotRequest {
            protocol: PROTOCOL,
            request,
        } = decode(&line.receive()?)?
        else {
            return Err("native snapshot expected exact Face request".into());
        };
        let c = &snapshot.credential;
        if request.credential_id != c.credential_id.as_str()
            || request.body_id != c.body_id
            || request.part_id != c.part_id
            || request.host_id != c.host_id
            || request.boot_id != c.boot_id
        {
            return Err("native snapshot request differs from authenticated Part".into());
        }
        let response = control::native_snapshot::face(
            state,
            &authorization.window_id,
            binding.clone(),
            request,
        )?;
        if let OwnerFaceSnapshotResponse::Snapshot {
            interactions_admitted,
            route,
            ..
        } = &response
        {
            if *interactions_admitted || route.is_some() {
                return Err("read-only snapshot must not claim Mask/Show authority".into());
            }
        }
        if serde_json::to_vec(&response)
            .map_err(|_| "native snapshot encoding")?
            .len()
            > MAX_OWNER_FACE_RESPONSE_BYTES
        {
            return Err("native snapshot canonical response bound".into());
        }
        line.send(&encode(&Out::FaceSnapshotResponse {
            protocol: PROTOCOL,
            response,
        })?)?;
        Ok(NativeSnapshotEvidence {
            session: selected.clone(),
            limits,
            window_id: authorization.window_id.clone(),
            credential_id: c.credential_id.as_str().into(),
            face_grant_id: grant.face_grant_id.clone(),
        })
    })();
    // Always retire the exact owner window/Part, including failed transport.
    let cleanup = if let Some(credential) = active {
        browser::leave(state, &authorization.window_id, credential).map(|_| ())
    } else {
        browser::abort(state, &authorization.window_id)
    };
    let cancel = browser::cancel(state, &authorization.window_id);
    let close = line.close();
    let mut error = result.as_ref().err().cloned();
    for (label, step) in [
        ("leave/abort", cleanup),
        ("cancel", cancel),
        ("close", close),
    ] {
        if let Err(detail) = step {
            error = Some(format!(
                "{};{label}:{detail}",
                error.unwrap_or_else(|| "snapshot served".into())
            ));
        }
    }
    match error {
        Some(error) => Err(error),
        None => result,
    }
}
fn decode(bytes: &[u8]) -> Result<In, String> {
    if bytes.is_empty() || bytes.len() > MAX_BROWSER_ADMISSION_FRAME_BYTES {
        return Err("native snapshot request bound".into());
    }
    serde_json::from_slice(bytes).map_err(|_| "native snapshot request encoding".into())
}
fn encode(frame: &Out) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(frame).map_err(|_| "native snapshot response encoding")?;
    if bytes.len() > MAX_BROWSER_ADMISSION_FRAME_BYTES {
        return Err("native snapshot response bound".into());
    }
    Ok(bytes)
}
