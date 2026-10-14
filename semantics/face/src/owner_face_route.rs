//! One exact owner-produced Face snapshot on an already authenticated carrier.
//!
//! A browser snapshot may include an owner-issued face route. Its
//! admitted Lines and current availability remain distinct from the Face;
//! the owner rechecks the credential and selected route before an action.

use alloc::{boxed::Box, string::String};
use conduit_body::{BodyId, PartId};
use conduit_core::{BootId, HostId};
use serde::{Deserialize, Serialize};

use crate::{Face, FaceContentId, RemoteOwnerMaskRouteSeal};

pub const OWNER_FACE_REQUEST_SCHEMA: &str = "conduit.face/owner-face-request@1";
pub const OWNER_FACE_RESPONSE_SCHEMA: &str = "conduit.face/owner-face-response@1";
/// Finite v1 owner-Face response profile. A Host route unable to carry this
/// whole response is ineligible; a larger Face is refused without truncation.
pub const MAX_OWNER_FACE_RESPONSE_BYTES: usize = 60 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerFaceSnapshotRequest {
    pub schema: String,
    pub credential_id: String,
    pub body_id: BodyId,
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub last_seen_revision: Option<u64>,
    pub last_seen_identity: Option<FaceContentId>,
}

impl OwnerFaceSnapshotRequest {
    pub fn has_exact_basis(&self) -> bool {
        self.schema == OWNER_FACE_REQUEST_SCHEMA
            && !self.credential_id.is_empty()
            && self.credential_id.len() <= 256
            && self.last_seen_revision.is_some() == self.last_seen_identity.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OwnerFaceSnapshotResponse {
    Snapshot {
        schema: String,
        face: Box<Face>,
        /// False until a typed semantic-interaction return route is admitted.
        interactions_admitted: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        route: Option<Box<RemoteOwnerMaskRouteSeal>>,
    },
    Unchanged {
        schema: String,
        revision: u64,
        identity: FaceContentId,
    },
    Refused {
        schema: String,
        code: String,
    },
}
