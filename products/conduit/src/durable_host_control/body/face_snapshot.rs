//! Remote peer-profile and authenticated local Face response bounds.
use super::*;

impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn owned_body_face(
        &self,
        request: &OwnerFaceSnapshotRequest,
    ) -> Result<OwnerFaceSnapshotResponse, String> {
        let HostSource::Body { owner, .. } = &self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        let face = owner.face_snapshot(request)?;
        let response = if request.last_seen_revision == Some(face.revision)
            && request.last_seen_identity.as_ref() == Some(&face.identity)
        {
            OwnerFaceSnapshotResponse::Unchanged {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                revision: face.revision,
                identity: face.identity,
            }
        } else {
            OwnerFaceSnapshotResponse::Snapshot {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                face: Box::new(face),
                interactions_admitted: false,
                route: None,
            }
        };
        let encoded = serde_json::to_vec(&response)
            .map_err(|error| format!("encode owner Face snapshot: {error}"))?;
        if encoded.len() > owner.native_face_response_bytes(request) {
            return Err("face-frame-pressure".into());
        }
        Ok(response)
    }

    pub(in crate::durable_host_control) fn owned_body_local_face(
        &self,
    ) -> Result<(Face, conduit_core::HostAdvertisement), String> {
        let HostSource::Body { owner, .. } = &self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        let face = owner.local_face_snapshot()?;
        if serde_json::to_vec(&face)
            .map_err(|error| format!("encode local owner Face: {error}"))?
            .len()
            > super::super::MAXIMUM_LOCAL_CONTROL_FRAME_BYTES / 2
        {
            return Err("face-frame-pressure".into());
        }
        Ok((face, owner.host.advertisement().clone()))
    }
}
