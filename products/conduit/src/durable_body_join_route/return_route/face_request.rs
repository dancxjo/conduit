//! Decode a read-only native Face request and its explicit return-route opt-in.

use conduit_presentation::OwnerFaceSnapshotRequest;
use serde::Deserialize;

pub(super) const FACE_REQUEST_SCHEMA: &str = "conduit.body/native-owner-face-request@1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FaceRequest {
    schema: String,
    request: OwnerFaceSnapshotRequest,
}

pub(super) fn decode_face_request(
    bytes: &[u8],
) -> Result<(OwnerFaceSnapshotRequest, bool), String> {
    if let Ok(wrapper) = serde_json::from_slice::<FaceRequest>(bytes) {
        if wrapper.schema != FACE_REQUEST_SCHEMA {
            return Err("unsupported native owner Face request".into());
        }
        return Ok((wrapper.request, true));
    }
    serde_json::from_slice(bytes)
        .map(|request| (request, false))
        .map_err(|error| format!("decode owner Face request: {error}"))
}
