use conduit_body::{BodyBiographyEvidence, BodyGraduationChoice};
use conduit_core::{ImplementationId, PlanId};

use crate::{project_body_biography, BodyBiographyProjection};

/// The Crèche currently exposes its durable evidence through one 32 KiB output
/// frame. Ordinary Patchbay entrances retain that bound before decoding.
pub const MAX_PATCHBAY_BODY_EVIDENCE_BYTES: usize = 32 * 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayBodyApplicationEntrance {
    Hosted {
        plan_id: PlanId,
        implementation_id: ImplementationId,
    },
    ExternalReader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayBodyAttachment {
    entrance: PatchbayBodyApplicationEntrance,
    evidence: BodyBiographyEvidence,
    projection: BodyBiographyProjection,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PatchbayBodyEntranceError {
    EmptyEvidence,
    EvidenceTooLarge,
    MalformedEvidence,
    InvalidEvidence,
    MissingGraduation,
    HostedPlacementMismatch,
}

impl PatchbayBodyAttachment {
    /// Opens one immutable Body-evidence snapshot at the Patchbay application
    /// boundary. Renderer and Host-composition adapters receive this validated
    /// attachment and must not decode or reinterpret the document themselves.
    pub fn open_serialized(
        encoded: &[u8],
        entrance: PatchbayBodyApplicationEntrance,
    ) -> Result<Self, PatchbayBodyEntranceError> {
        if encoded.is_empty() {
            return Err(PatchbayBodyEntranceError::EmptyEvidence);
        }
        if encoded.len() > MAX_PATCHBAY_BODY_EVIDENCE_BYTES {
            return Err(PatchbayBodyEntranceError::EvidenceTooLarge);
        }
        let evidence: BodyBiographyEvidence = serde_json::from_slice(encoded)
            .map_err(|_| PatchbayBodyEntranceError::MalformedEvidence)?;
        let projection = project_body_biography(&evidence)
            .map_err(|_| PatchbayBodyEntranceError::InvalidEvidence)?;
        let graduation = evidence
            .graduation
            .as_ref()
            .ok_or(PatchbayBodyEntranceError::MissingGraduation)?;

        if let PatchbayBodyApplicationEntrance::Hosted {
            plan_id,
            implementation_id,
        } = &entrance
        {
            let exact_placement = graduation.choice == BodyGraduationChoice::HostedPatchbay
                && graduation.patchbay_plan_id.as_ref() == Some(plan_id)
                && graduation.patchbay_implementation_id.as_ref() == Some(implementation_id);
            if !exact_placement {
                return Err(PatchbayBodyEntranceError::HostedPlacementMismatch);
            }
        }

        Ok(Self {
            entrance,
            evidence,
            projection,
        })
    }

    pub fn entrance(&self) -> &PatchbayBodyApplicationEntrance {
        &self.entrance
    }

    pub fn evidence(&self) -> &BodyBiographyEvidence {
        &self.evidence
    }

    pub fn projection(&self) -> &BodyBiographyProjection {
        &self.projection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{
        AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyGraduationEvidence,
        BodyMembership, MembershipProofId, PartId,
    };
    use conduit_core::{
        bind_sign, BootId, CheckedFormId, HostId, OfferGeneration, SignId, SourceDocumentId,
    };

    const HOSTED_PLAN: &str = "plan/creche-hosted-patchbay";
    const HOSTED_IMPLEMENTATION: &str = "browser/patchbay-surface@1";

    fn graduated_evidence(choice: BodyGraduationChoice) -> BodyBiographyEvidence {
        let host_id = HostId::from("browser/creche");
        let boot_id = BootId::from("browser-boot/creche");
        let birth_sign = bind_sign(&host_id, &boot_id, None, 1).sign_id;
        let body = Body::born(
            SourceDocumentId::from("source/creche-morse-network"),
            CheckedFormId::from("checked/creche-morse-network"),
            1,
            birth_sign,
        )
        .unwrap();
        let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
        let part_id = PartId::bind(&body.body_id, "creche/here", 1).unwrap();
        let proof_id = MembershipProofId::bind("proof/creche-here").unwrap();
        let admitted_sign = bind_sign(&host_id, &boot_id, None, 2).sign_id;
        let admitted_change = membership
            .admit(
                &body.body_id,
                membership.revision,
                part_id.clone(),
                proof_id.clone(),
                admitted_sign,
            )
            .unwrap();
        let joined_sign = bind_sign(&host_id, &boot_id, None, 3).sign_id;
        let joined_change = membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part_id,
                AuthenticatedHostObservation {
                    host_id: host_id.clone(),
                    boot_id: boot_id.clone(),
                    offer_generation: OfferGeneration(1),
                    proof_id,
                    sequence: 1,
                },
                joined_sign,
            )
            .unwrap();
        let mut evidence = BodyBiographyEvidence::born(
            body,
            BodyMembership::new(membership.body_id.clone()).unwrap(),
            "Roseau".into(),
        )
        .unwrap();
        evidence
            .append_membership_events(membership, &[(admitted_change, 2), (joined_change, 3)])
            .unwrap();
        let (patchbay_plan_id, patchbay_implementation_id) = match choice {
            BodyGraduationChoice::HostedPatchbay => (
                Some(PlanId::from(HOSTED_PLAN)),
                Some(ImplementationId::from(HOSTED_IMPLEMENTATION)),
            ),
            BodyGraduationChoice::ExternalReader => (None, None),
        };
        evidence
            .graduate(BodyGraduationEvidence {
                body_id: evidence.body_id.clone(),
                sequence: 4,
                sign_id: SignId::from("sign/creche-graduated"),
                choice,
                patchbay_plan_id,
                patchbay_implementation_id,
            })
            .unwrap();
        evidence
    }

    fn encoded(choice: BodyGraduationChoice) -> Vec<u8> {
        serde_json::to_vec(&graduated_evidence(choice)).unwrap()
    }

    #[test]
    fn the_same_durable_document_opens_through_hosted_and_external_entrances() {
        let encoded = encoded(BodyGraduationChoice::HostedPatchbay);
        let hosted = PatchbayBodyAttachment::open_serialized(
            &encoded,
            PatchbayBodyApplicationEntrance::Hosted {
                plan_id: PlanId::from(HOSTED_PLAN),
                implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
            },
        )
        .unwrap();
        let external = PatchbayBodyAttachment::open_serialized(
            &encoded,
            PatchbayBodyApplicationEntrance::ExternalReader,
        )
        .unwrap();

        assert_eq!(hosted.evidence(), external.evidence());
        assert_eq!(hosted.projection(), external.projection());
        assert_eq!(hosted.projection().entries.len(), 4);
        assert_eq!(
            hosted.projection().body_id,
            graduated_evidence(BodyGraduationChoice::HostedPatchbay).body_id
        );
    }

    #[test]
    fn an_external_reader_opens_a_body_that_graduated_without_hosted_patchbay() {
        let encoded = encoded(BodyGraduationChoice::ExternalReader);
        let attachment = PatchbayBodyAttachment::open_serialized(
            &encoded,
            PatchbayBodyApplicationEntrance::ExternalReader,
        )
        .unwrap();

        assert!(matches!(
            attachment.entrance(),
            PatchbayBodyApplicationEntrance::ExternalReader
        ));
        assert!(matches!(
            attachment.evidence().graduation.as_ref().unwrap().choice,
            BodyGraduationChoice::ExternalReader
        ));
    }

    #[test]
    fn invalid_wrongly_placed_and_oversized_documents_fail_closed() {
        assert_eq!(
            PatchbayBodyAttachment::open_serialized(
                &[],
                PatchbayBodyApplicationEntrance::ExternalReader,
            ),
            Err(PatchbayBodyEntranceError::EmptyEvidence)
        );
        assert_eq!(
            PatchbayBodyAttachment::open_serialized(
                b"{not-json",
                PatchbayBodyApplicationEntrance::ExternalReader,
            ),
            Err(PatchbayBodyEntranceError::MalformedEvidence)
        );

        let mut unsupported = graduated_evidence(BodyGraduationChoice::HostedPatchbay);
        unsupported.schema = "conduit.body/biography-evidence@future".into();
        assert_eq!(
            PatchbayBodyAttachment::open_serialized(
                &serde_json::to_vec(&unsupported).unwrap(),
                PatchbayBodyApplicationEntrance::ExternalReader,
            ),
            Err(PatchbayBodyEntranceError::InvalidEvidence)
        );

        let mut ungraduated = graduated_evidence(BodyGraduationChoice::HostedPatchbay);
        ungraduated.graduation = None;
        ungraduated.records.pop();
        assert!(ungraduated.validate().is_ok());
        assert_eq!(
            PatchbayBodyAttachment::open_serialized(
                &serde_json::to_vec(&ungraduated).unwrap(),
                PatchbayBodyApplicationEntrance::ExternalReader,
            ),
            Err(PatchbayBodyEntranceError::MissingGraduation)
        );

        let mut tampered = graduated_evidence(BodyGraduationChoice::HostedPatchbay);
        tampered.records[1].sequence = tampered.records[0].sequence;
        assert_eq!(
            PatchbayBodyAttachment::open_serialized(
                &serde_json::to_vec(&tampered).unwrap(),
                PatchbayBodyApplicationEntrance::ExternalReader,
            ),
            Err(PatchbayBodyEntranceError::InvalidEvidence)
        );

        assert_eq!(
            PatchbayBodyAttachment::open_serialized(
                &encoded(BodyGraduationChoice::HostedPatchbay),
                PatchbayBodyApplicationEntrance::Hosted {
                    plan_id: PlanId::from("plan/not-the-graduation-plan"),
                    implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
                },
            ),
            Err(PatchbayBodyEntranceError::HostedPlacementMismatch)
        );
        assert_eq!(
            PatchbayBodyAttachment::open_serialized(
                &encoded(BodyGraduationChoice::HostedPatchbay),
                PatchbayBodyApplicationEntrance::Hosted {
                    plan_id: PlanId::from(HOSTED_PLAN),
                    implementation_id: ImplementationId::from("browser/not-patchbay@1"),
                },
            ),
            Err(PatchbayBodyEntranceError::HostedPlacementMismatch)
        );
        assert_eq!(
            PatchbayBodyAttachment::open_serialized(
                &encoded(BodyGraduationChoice::ExternalReader),
                PatchbayBodyApplicationEntrance::Hosted {
                    plan_id: PlanId::from(HOSTED_PLAN),
                    implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
                },
            ),
            Err(PatchbayBodyEntranceError::HostedPlacementMismatch)
        );
        assert_eq!(
            PatchbayBodyAttachment::open_serialized(
                &vec![b' '; MAX_PATCHBAY_BODY_EVIDENCE_BYTES + 1],
                PatchbayBodyApplicationEntrance::ExternalReader,
            ),
            Err(PatchbayBodyEntranceError::EvidenceTooLarge)
        );
    }

    #[test]
    fn dropping_patchbay_cannot_mutate_or_truncate_the_source_evidence() {
        let source = graduated_evidence(BodyGraduationChoice::ExternalReader);
        let original = source.clone();
        let encoded = serde_json::to_vec(&source).unwrap();
        {
            let attachment = PatchbayBodyAttachment::open_serialized(
                &encoded,
                PatchbayBodyApplicationEntrance::ExternalReader,
            )
            .unwrap();
            assert_eq!(attachment.evidence(), &source);
        }

        assert_eq!(source, original);
        assert_eq!(
            serde_json::from_slice::<BodyBiographyEvidence>(&encoded).unwrap(),
            original
        );
    }
}
