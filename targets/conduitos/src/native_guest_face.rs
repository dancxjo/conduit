//! One owner-produced Face snapshot for an already admitted native Part.
//!
//! A pinned owner Line authenticates the producer. This module checks that the
//! received immutable Face describes the Body named by the exact admission
//! receipt. A separate, bounded return grant is required before actions.

use alloc::{format, string::String, vec::Vec};
use conduit_body::{PortableAdmissionReceipt, SPAWN_ADMISSION_RECEIPT_SCHEMA};
use conduit_face::{
    Face, FaceRole, OWNER_FACE_RESPONSE_SCHEMA, OwnerFaceSnapshotResponse, RemoteOwnerMaskRouteSeal,
};
use conduit_plot::rust_binding::NativeRustBinding;

use crate::native_guest_part::NativeGuestPart;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeGuestFace {
    credential_id: String,
    face: Face,
    interactions_admitted: bool,
    route: Option<RemoteOwnerMaskRouteSeal>,
    history_look: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestFaceRefusal {
    ResponseSchema,
    OwnerRefused(String),
    UnexpectedUnchanged,
    InteractionRoute,
    ReceiptBasis,
    Face,
    BodyBasis,
    MaskRoute,
    LookInput,
}

impl GuestFaceRefusal {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ResponseSchema => "native-owner-face-response-invalid",
            Self::OwnerRefused(_) => "native-owner-face-refused",
            Self::UnexpectedUnchanged => "native-owner-face-unexpected-unchanged",
            Self::InteractionRoute => "native-owner-face-interaction-route-unadmitted",
            Self::ReceiptBasis => "native-owner-face-receipt-invalid",
            Self::Face => "native-owner-face-invalid",
            Self::BodyBasis => "native-owner-face-body-mismatch",
            Self::MaskRoute => "native-owner-mask-route-invalid",
            Self::LookInput => "native-owner-mask-look-invalid",
        }
    }
}

impl NativeGuestFace {
    pub fn from_owner_response(
        receipt: &PortableAdmissionReceipt,
        response: OwnerFaceSnapshotResponse,
    ) -> Result<Self, GuestFaceRefusal> {
        if receipt.schema != SPAWN_ADMISSION_RECEIPT_SCHEMA
            || !receipt.membership_admitted
            || receipt.plan_created
            || receipt.play_created
            || receipt.credential.host_id != receipt.host_advertisement.host_id
            || receipt.credential.boot_id != receipt.host_advertisement.boot_id
        {
            return Err(GuestFaceRefusal::ReceiptBasis);
        }
        let (face, interactions_admitted, route) = match response {
            OwnerFaceSnapshotResponse::Snapshot {
                schema,
                face,
                interactions_admitted,
                route,
            } if schema == OWNER_FACE_RESPONSE_SCHEMA => {
                (*face, interactions_admitted, route.map(|route| *route))
            }
            OwnerFaceSnapshotResponse::Refused { schema, code }
                if schema == OWNER_FACE_RESPONSE_SCHEMA
                    && !code.is_empty()
                    && code.len() <= 256 =>
            {
                return Err(GuestFaceRefusal::OwnerRefused(code));
            }
            OwnerFaceSnapshotResponse::Unchanged { schema, .. }
                if schema == OWNER_FACE_RESPONSE_SCHEMA =>
            {
                return Err(GuestFaceRefusal::UnexpectedUnchanged);
            }
            _ => return Err(GuestFaceRefusal::ResponseSchema),
        };
        face.validate().map_err(|_| GuestFaceRefusal::Face)?;
        let body = &receipt.credential.body_id;
        let body_subject = format!("body/{}", body.as_str());
        if face.revision == 0
            || face.basis.body_id.as_ref() != Some(body)
            || !face
                .subjects
                .iter()
                .any(|subject| subject.role == FaceRole::Body && subject.identity == body_subject)
        {
            return Err(GuestFaceRefusal::BodyBasis);
        }
        if interactions_admitted != route.is_some() {
            return Err(GuestFaceRefusal::MaskRoute);
        }
        if let Some(route) = &route {
            route
                .validate_mask_host_offer(&receipt.host_advertisement)
                .map_err(|_| GuestFaceRefusal::MaskRoute)?;
            if route.body_id != *body
                || route.face_id != face.identity
                || route.face_revision != face.revision
                || route.face_basis != face.basis
            {
                return Err(GuestFaceRefusal::MaskRoute);
            }
        }
        Ok(Self {
            credential_id: receipt.credential.credential_id.as_str().into(),
            face,
            interactions_admitted,
            route,
            history_look: None,
        })
    }

    pub fn matches_part(&self, part: &NativeGuestPart) -> bool {
        self.credential_id == part.credential().credential_id.as_str()
            && self.face.basis.body_id.as_ref() == Some(&part.credential().body_id)
    }

    pub fn face(&self) -> &Face {
        &self.face
    }

    pub fn interactions_admitted(&self) -> bool {
        self.interactions_admitted
    }

    pub fn route(&self) -> Option<&RemoteOwnerMaskRouteSeal> {
        self.route.as_ref()
    }

    /// Retain independent presentation intent only after reproducing this exact
    /// owner-issued Mask Plan on the independently admitted Host offer.
    pub fn with_history_look(
        mut self,
        receipt: &PortableAdmissionReceipt,
        look: &conduit_face::LookInfo,
    ) -> Result<Self, GuestFaceRefusal> {
        if self.credential_id != receipt.credential.credential_id.as_str()
            || self.face.basis.body_id.as_ref() != Some(&receipt.credential.body_id)
        {
            return Err(GuestFaceRefusal::ReceiptBasis);
        }
        let bytes = look
            .clone()
            .encode()
            .map_err(|_| GuestFaceRefusal::LookInput)?;
        if bytes.len() > conduit_face::MAX_LOOK_BYTES {
            return Err(GuestFaceRefusal::LookInput);
        }
        let (expected, _) =
            conduit_conduitos_mask_offer::prepare_history_component_stage_with_look_from_offer(
                &receipt.host_advertisement,
                look,
            )
            .map_err(|_| GuestFaceRefusal::LookInput)?;
        if self.route.as_ref().map(|r| &r.planned_mask) != Some(&expected.planned_mask) {
            return Err(GuestFaceRefusal::MaskRoute);
        }
        self.history_look = Some(bytes);
        Ok(self)
    }

    /// A refresh of the same prepared Mask retains already validated intent;
    /// it does not compile an unchanged Source again for every observation.
    #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
    pub(crate) fn retain_history_look_from(
        mut self,
        prior: &Self,
    ) -> Result<Self, GuestFaceRefusal> {
        if self.credential_id != prior.credential_id
            || self.face.basis.body_id != prior.face.basis.body_id
            || self.route.as_ref().map(|r| &r.planned_mask)
                != prior.route.as_ref().map(|r| &r.planned_mask)
        {
            return Err(GuestFaceRefusal::MaskRoute);
        }
        self.history_look = prior.history_look.clone();
        Ok(self)
    }

    pub fn history_look(&self) -> Result<Option<conduit_face::LookInfo>, GuestFaceRefusal> {
        self.history_look
            .as_deref()
            .map(|bytes| {
                conduit_face::LookInfo::decode(bytes).map_err(|_| GuestFaceRefusal::LookInput)
            })
            .transpose()
    }

    pub fn into_face(self) -> Face {
        self.face
    }
}
