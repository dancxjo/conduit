//! Bounded decoding and exact entrance validation for durable Body evidence.

use conduit_body::{BodyBiographyEvidence, BodyGraduationChoice};
use conduit_core::{ImplementationId, PlanId};

pub const MAX_BODY_ATTACHMENT_EVIDENCE_BYTES: usize = 32 * 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyEvidenceEntrance {
    Hosted {
        plan_id: PlanId,
        implementation_id: ImplementationId,
    },
    ExternalReader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyEvidenceAttachment {
    entrance: BodyEvidenceEntrance,
    evidence: BodyBiographyEvidence,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyEvidenceEntranceError {
    EmptyEvidence,
    EvidenceTooLarge,
    MalformedEvidence,
    InvalidEvidence,
    MissingGraduation,
    HostedPlacementMismatch,
}

impl BodyEvidenceAttachment {
    pub fn open_serialized(
        encoded: &[u8],
        entrance: BodyEvidenceEntrance,
    ) -> Result<Self, BodyEvidenceEntranceError> {
        if encoded.is_empty() {
            return Err(BodyEvidenceEntranceError::EmptyEvidence);
        }
        if encoded.len() > MAX_BODY_ATTACHMENT_EVIDENCE_BYTES {
            return Err(BodyEvidenceEntranceError::EvidenceTooLarge);
        }
        let evidence: BodyBiographyEvidence = serde_json::from_slice(encoded)
            .map_err(|_| BodyEvidenceEntranceError::MalformedEvidence)?;
        evidence
            .validate()
            .map_err(|_| BodyEvidenceEntranceError::InvalidEvidence)?;
        if let BodyEvidenceEntrance::Hosted {
            plan_id,
            implementation_id,
        } = &entrance
        {
            let graduation = evidence
                .graduation
                .as_ref()
                .ok_or(BodyEvidenceEntranceError::MissingGraduation)?;
            let exact_placement = graduation.choice == BodyGraduationChoice::HostedReader
                && graduation.reader_plan_id.as_ref() == Some(plan_id)
                && graduation.reader_implementation_id.as_ref() == Some(implementation_id);
            if !exact_placement {
                return Err(BodyEvidenceEntranceError::HostedPlacementMismatch);
            }
        }

        Ok(Self { entrance, evidence })
    }

    pub fn entrance(&self) -> &BodyEvidenceEntrance {
        &self.entrance
    }

    pub fn evidence(&self) -> &BodyBiographyEvidence {
        &self.evidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{
        Body, BodyBiographyEvidence, BodyGraduationEvidence, BodyMembership, BodyWorkset,
        ResidentPlot,
    };
    use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};

    const HOSTED_PLAN: &str = "plan/body-hosted-surface";
    const HOSTED_IMPLEMENTATION: &str = "browser/body-surface@1";

    fn evidence(choice: BodyGraduationChoice) -> BodyBiographyEvidence {
        let plot = ResidentPlot::new(
            SourceDocumentId::from("source/body"),
            CheckedPlotId::from("checked/body"),
        );
        let body = Body::born_with_plots(
            BodyWorkset::one(plot).unwrap(),
            1,
            SignId::from("sign/born"),
        )
        .unwrap();
        let membership = BodyMembership::new(body.body_id.clone()).unwrap();
        let mut evidence = BodyBiographyEvidence::born(body, membership, "Talvi".into()).unwrap();
        let (plan_id, implementation_id) = match choice {
            BodyGraduationChoice::HostedReader => (
                Some(PlanId::from(HOSTED_PLAN)),
                Some(ImplementationId::from(HOSTED_IMPLEMENTATION)),
            ),
            BodyGraduationChoice::ExternalReader => (None, None),
        };
        evidence
            .graduate(BodyGraduationEvidence {
                body_id: evidence.body_id.clone(),
                sequence: 2,
                sign_id: SignId::from("sign/graduated"),
                choice,
                reader_plan_id: plan_id,
                reader_implementation_id: implementation_id,
            })
            .unwrap();
        evidence
    }

    fn encoded(choice: BodyGraduationChoice) -> Vec<u8> {
        serde_json::to_vec(&evidence(choice)).unwrap()
    }

    #[test]
    fn exact_hosted_and_external_entrances_open_the_same_durable_evidence() {
        let encoded = encoded(BodyGraduationChoice::HostedReader);
        let durable_wire = core::str::from_utf8(&encoded).unwrap();
        assert!(durable_wire.contains("\"choice\":\"HostedPatchbay\""));
        assert!(durable_wire.contains("\"patchbay_plan_id\""));
        assert!(durable_wire.contains("\"patchbay_implementation_id\""));
        assert!(!durable_wire.contains("reader_plan_id"));
        assert!(!durable_wire.contains("reader_implementation_id"));
        let hosted = BodyEvidenceAttachment::open_serialized(
            &encoded,
            BodyEvidenceEntrance::Hosted {
                plan_id: PlanId::from(HOSTED_PLAN),
                implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
            },
        )
        .unwrap();
        let external =
            BodyEvidenceAttachment::open_serialized(&encoded, BodyEvidenceEntrance::ExternalReader)
                .unwrap();
        assert_eq!(hosted.evidence(), external.evidence());
    }

    #[test]
    fn hosted_entrance_refuses_missing_or_mismatched_graduation_placement() {
        let wrong = BodyEvidenceAttachment::open_serialized(
            &encoded(BodyGraduationChoice::HostedReader),
            BodyEvidenceEntrance::Hosted {
                plan_id: PlanId::from("plan/wrong"),
                implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
            },
        );
        assert_eq!(
            wrong,
            Err(BodyEvidenceEntranceError::HostedPlacementMismatch)
        );
        let external = BodyEvidenceAttachment::open_serialized(
            &encoded(BodyGraduationChoice::ExternalReader),
            BodyEvidenceEntrance::Hosted {
                plan_id: PlanId::from(HOSTED_PLAN),
                implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
            },
        );
        assert_eq!(
            external,
            Err(BodyEvidenceEntranceError::HostedPlacementMismatch)
        );
    }

    #[test]
    fn malformed_invalid_and_oversized_evidence_refuses_before_attachment() {
        assert_eq!(
            BodyEvidenceAttachment::open_serialized(&[], BodyEvidenceEntrance::ExternalReader),
            Err(BodyEvidenceEntranceError::EmptyEvidence)
        );
        assert_eq!(
            BodyEvidenceAttachment::open_serialized(
                b"{not-json",
                BodyEvidenceEntrance::ExternalReader,
            ),
            Err(BodyEvidenceEntranceError::MalformedEvidence)
        );
        let mut invalid = evidence(BodyGraduationChoice::ExternalReader);
        invalid.schema = "conduit.body/biography-evidence@future".into();
        assert_eq!(
            BodyEvidenceAttachment::open_serialized(
                &serde_json::to_vec(&invalid).unwrap(),
                BodyEvidenceEntrance::ExternalReader,
            ),
            Err(BodyEvidenceEntranceError::InvalidEvidence)
        );
        assert_eq!(
            BodyEvidenceAttachment::open_serialized(
                &vec![b' '; MAX_BODY_ATTACHMENT_EVIDENCE_BYTES + 1],
                BodyEvidenceEntrance::ExternalReader,
            ),
            Err(BodyEvidenceEntranceError::EvidenceTooLarge)
        );
    }
}
