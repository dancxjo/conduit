//! One owner-produced Face snapshot for an already admitted native Part.
//!
//! A pinned owner Line authenticates the producer. This module checks that the
//! received immutable Face describes the Body named by the exact admission
//! receipt. A separate, bounded return grant is required before actions.

use alloc::{format, string::String};
use conduit_body::{PortableAdmissionReceipt, SPAWN_ADMISSION_RECEIPT_SCHEMA};
use conduit_presentation::{
    OWNER_FACE_RESPONSE_SCHEMA, OwnerFaceSnapshotResponse, Presentation, PresentationRole,
    RemoteOwnerMaskRouteSeal,
};

use crate::native_guest_part::NativeGuestPart;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeGuestFace {
    credential_id: String,
    presentation: Presentation,
    interactions_admitted: bool,
    route: Option<RemoteOwnerMaskRouteSeal>,
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
        let (presentation, interactions_admitted, route) = match response {
            OwnerFaceSnapshotResponse::Snapshot {
                schema,
                presentation,
                interactions_admitted,
                route,
            } if schema == OWNER_FACE_RESPONSE_SCHEMA => (
                *presentation,
                interactions_admitted,
                route.map(|route| *route),
            ),
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
        presentation
            .validate()
            .map_err(|_| GuestFaceRefusal::Face)?;
        let body = &receipt.credential.body_id;
        let body_subject = format!("body/{}", body.as_str());
        if presentation.revision == 0
            || presentation.basis.body_id.as_ref() != Some(body)
            || !presentation.subjects.iter().any(|subject| {
                subject.role == PresentationRole::Body && subject.identity == body_subject
            })
        {
            return Err(GuestFaceRefusal::BodyBasis);
        }
        if let Some(route) = &route {
            route
                .validate_mask_host_offer(&receipt.host_advertisement)
                .map_err(|_| GuestFaceRefusal::MaskRoute)?;
            if route.body_id != *body
                || route.face_id != presentation.identity
                || route.face_revision != presentation.revision
                || route.face_basis != presentation.basis
            {
                return Err(GuestFaceRefusal::MaskRoute);
            }
        }
        Ok(Self {
            credential_id: receipt.credential.credential_id.as_str().into(),
            presentation,
            interactions_admitted,
            route,
        })
    }

    pub fn matches_part(&self, part: &NativeGuestPart) -> bool {
        self.credential_id == part.credential().credential_id.as_str()
            && self.presentation.basis.body_id.as_ref() == Some(&part.credential().body_id)
    }

    pub fn presentation(&self) -> &Presentation {
        &self.presentation
    }

    pub fn interactions_admitted(&self) -> bool {
        self.interactions_admitted
    }

    pub fn route(&self) -> Option<&RemoteOwnerMaskRouteSeal> {
        self.route.as_ref()
    }

    pub fn into_presentation(self) -> Presentation {
        self.presentation
    }
}
