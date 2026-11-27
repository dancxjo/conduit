//! Explicit caller-selected owner Face egress, separate from the default profile.
//! Limits select storage; they neither admit a credential nor authorize a route.
use super::*;
use std::io::{Cursor, Write};

#[derive(Debug, Clone, Copy)]
pub struct OwnerFaceEgressProfile<'a> {
    pub execution_profile_id: &'a str,
    pub maximum_face_bytes: usize,
    pub maximum_route_bytes: usize,
    pub maximum_response_bytes: usize,
    pub maximum_frame_bytes: usize,
}

/// The owner must select this profile only after authenticating the caller and
/// preparing its actual carrier buffers. All other frames use the default
/// encoder. This explicit path has its own 1 MiB hard frame ceiling; it
/// does not raise the default Face or general admission limits. Seal
/// verification may allocate during owner preparation; it is not a Step or
/// allocator-confinement proof. The ordinary Face/Show identities remain.
pub fn encode_browser_admission_face_frame_for_profile(
    frame: &BrowserAdmissionEgress,
    profile: &OwnerFaceEgressProfile<'_>,
    output: &mut [u8],
) -> Result<usize, BrowserAdmissionFrameError> {
    let fail = BrowserAdmissionFrameError::InvalidFaceSnapshot;
    if profile.execution_profile_id.is_empty()
        || profile.execution_profile_id.len() > 256
        || profile.maximum_face_bytes == 0
        || profile.maximum_face_bytes > conduit_face::json_model_preflight::MAX_FACE_JSON_BYTES
        || profile.maximum_route_bytes == 0
        || profile.maximum_route_bytes > 64 * 1024
        || profile.maximum_response_bytes < profile.maximum_face_bytes
        || profile.maximum_frame_bytes < profile.maximum_response_bytes
        || profile.maximum_frame_bytes > 1024 * 1024
    {
        return Err(fail);
    }
    let BrowserAdmissionEgress::FaceSnapshotResponse { protocol, response } = frame else {
        return Err(fail);
    };
    let OwnerFaceSnapshotResponse::Snapshot {
        schema,
        face,
        route: Some(route),
        ..
    } = response
    else {
        return Err(fail);
    };
    if *protocol != BROWSER_ADMISSION_PROTOCOL
        || schema != OWNER_FACE_RESPONSE_SCHEMA
        || route
            .planned_mask
            .show_placement()
            .execution_profile_id
            .as_str()
            != profile.execution_profile_id
        || route.face_id != face.identity
        || route.face_revision != face.revision
        || route.face_basis != face.basis
    {
        return Err(fail);
    }
    face.validate().map_err(|_| fail)?;
    // Establish the existing Plan codec's conditional workspace/extent fence
    // before seal verification can serialize its compressed representation.
    let plan_maximum = output.len().min(256 * 1024);
    let plan_length = write(&route.planned_mask.plan, &mut output[..plan_maximum])?;
    conduit_face::PlannedMaskPlot::generic_json_plan_preparation_bytes(&output[..plan_length])
        .map_err(|_| fail)?;
    route.verify_seal().map_err(|_| fail)?;
    // First use the actual bounded output as preflight scratch. The whole
    // canonical Face grammar is checked before selecting this renderer profile;
    // no owned serde tree or growing serialization Vec is introduced here.
    let face_maximum = output.len().min(profile.maximum_face_bytes);
    let face_length = write(face, &mut output[..face_maximum])?;
    conduit_face::json_model_preflight::generic_face_json_shape(&output[..face_length])
        .map_err(|_| fail)?;
    count(route, profile.maximum_route_bytes)?;
    count(response, profile.maximum_response_bytes)?;
    let maximum = output.len().min(profile.maximum_frame_bytes);
    write(frame, &mut output[..maximum])
}

fn write(value: &impl Serialize, output: &mut [u8]) -> Result<usize, BrowserAdmissionFrameError> {
    let mut cursor = Cursor::new(output);
    serde_json::to_writer(&mut cursor, value)
        .map_err(|_| BrowserAdmissionFrameError::OutputTooSmall)?;
    usize::try_from(cursor.position()).map_err(|_| BrowserAdmissionFrameError::OutputTooSmall)
}
struct Count {
    length: usize,
    maximum: usize,
}
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("Face frame extent"))?;
        if self.length > self.maximum {
            return Err(std::io::Error::other("Face frame extent"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn count(value: &impl Serialize, maximum: usize) -> Result<(), BrowserAdmissionFrameError> {
    serde_json::to_writer(Count { length: 0, maximum }, value)
        .map_err(|_| BrowserAdmissionFrameError::InvalidFaceSnapshot)
}

#[cfg(test)]
#[path = "face_profile/tests.rs"]
mod tests;
