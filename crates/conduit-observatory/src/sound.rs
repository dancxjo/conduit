//! Bounded read-only explanation of sound realization choices.

use alloc::{format, string::String, vec::Vec};
use conduit_core::{
    ActivePlayId, BootId, CapabilityId, ExecutionProfileId, FormIdentity, HostId, ImplementationId,
    KindId, PlanId,
};
use serde::{Deserialize, Serialize};

pub const SOUND_INSPECTION_SCHEMA: &str = "conduit.observatory/sound-realization@1";
pub const MAXIMUM_SOUND_CANDIDATES: usize = 8;
pub const MAXIMUM_SOUND_STAGES: usize = 4;
pub const MAXIMUM_SOUND_REASON_BYTES: usize = 96;
pub const MAXIMUM_SOUND_RENDER_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SoundProofClass {
    DeterministicReference,
    HostedLiveDevice,
    FreestandingEmulator,
    PhysicalPeteCreateHil,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum SoundCandidateStatus {
    Compatible,
    Incompatible { reason: String },
    MissingRequiredProof { required: SoundProofClass },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SoundRealizationRoute {
    Direct,
    Recursive { stages: Vec<KindId> },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SoundCandidateInspection {
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub execution_profile_id: ExecutionProfileId,
    pub proof_class: SoundProofClass,
    pub status: SoundCandidateStatus,
    pub route: SoundRealizationRoute,
    pub host_id: Option<HostId>,
    pub boot_id: Option<BootId>,
    pub selected_plan_id: Option<PlanId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SoundRealizationInspection {
    pub schema: String,
    pub form: FormIdentity,
    pub requirement_profile_id: String,
    pub candidates: Vec<SoundCandidateInspection>,
    pub selected_capability_id: Option<CapabilityId>,
    pub active_play_id: Option<ActivePlayId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoundInspectionError {
    WrongSchema,
    EmptyProfile,
    CandidateBound,
    DuplicateCandidate,
    ReasonBound,
    RouteBound,
    PartialHostIdentity,
    InvalidSelection,
    InvalidActivePlay,
    RenderBound,
}

pub fn validate_sound_inspection(
    inspection: &SoundRealizationInspection,
) -> Result<(), SoundInspectionError> {
    if inspection.schema != SOUND_INSPECTION_SCHEMA {
        return Err(SoundInspectionError::WrongSchema);
    }
    if inspection.requirement_profile_id.is_empty() {
        return Err(SoundInspectionError::EmptyProfile);
    }
    if inspection.candidates.is_empty() || inspection.candidates.len() > MAXIMUM_SOUND_CANDIDATES {
        return Err(SoundInspectionError::CandidateBound);
    }
    for (index, candidate) in inspection.candidates.iter().enumerate() {
        if inspection.candidates[..index]
            .iter()
            .any(|other| other.capability_id == candidate.capability_id)
        {
            return Err(SoundInspectionError::DuplicateCandidate);
        }
        if let SoundCandidateStatus::Incompatible { reason } = &candidate.status {
            if reason.is_empty() || reason.len() > MAXIMUM_SOUND_REASON_BYTES {
                return Err(SoundInspectionError::ReasonBound);
            }
        }
        if let SoundRealizationRoute::Recursive { stages } = &candidate.route {
            if stages.is_empty() || stages.len() > MAXIMUM_SOUND_STAGES {
                return Err(SoundInspectionError::RouteBound);
            }
        }
        if candidate.host_id.is_some() != candidate.boot_id.is_some() {
            return Err(SoundInspectionError::PartialHostIdentity);
        }
    }
    match &inspection.selected_capability_id {
        Some(selected) => {
            let candidate = inspection
                .candidates
                .iter()
                .find(|candidate| &candidate.capability_id == selected)
                .ok_or(SoundInspectionError::InvalidSelection)?;
            if !matches!(candidate.status, SoundCandidateStatus::Compatible)
                || candidate.host_id.is_none()
                || candidate.selected_plan_id.is_none()
            {
                return Err(SoundInspectionError::InvalidSelection);
            }
        }
        None if inspection.active_play_id.is_some() => {
            return Err(SoundInspectionError::InvalidActivePlay);
        }
        None => {}
    }
    if inspection.active_play_id.is_some() && inspection.selected_capability_id.is_none() {
        return Err(SoundInspectionError::InvalidActivePlay);
    }
    Ok(())
}

pub fn render_sound_inspection(
    inspection: &SoundRealizationInspection,
) -> Result<String, SoundInspectionError> {
    validate_sound_inspection(inspection)?;
    let mut rendered = format!(
        "Sound realization\nForm {}\nRequirement {}\n",
        inspection.form.source_document_id.as_str(),
        inspection.requirement_profile_id
    );
    for candidate in &inspection.candidates {
        let status = match &candidate.status {
            SoundCandidateStatus::Compatible => "compatible".into(),
            SoundCandidateStatus::Incompatible { reason } => format!("unsupported: {reason}"),
            SoundCandidateStatus::MissingRequiredProof { required } => {
                format!("missing proof: {required:?}")
            }
        };
        let route = match &candidate.route {
            SoundRealizationRoute::Direct => "direct".into(),
            SoundRealizationRoute::Recursive { stages } => stages
                .iter()
                .map(|stage| stage.as_str())
                .collect::<Vec<_>>()
                .join(" -> "),
        };
        rendered.push_str(&format!(
            "{} | {} | {} | {}\n",
            candidate.capability_id.as_str(),
            status,
            route,
            candidate.implementation_id.as_str()
        ));
    }
    if let Some(selected) = &inspection.selected_capability_id {
        rendered.push_str(&format!("Selected {}\n", selected.as_str()));
    }
    if let Some(play) = &inspection.active_play_id {
        rendered.push_str(&format!("Play {}\n", play.as_str()));
    }
    if rendered.len() > MAXIMUM_SOUND_RENDER_BYTES {
        return Err(SoundInspectionError::RenderBound);
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use conduit_core::{CheckedFormId, ExpandedFormId, SourceDocumentId};

    fn form() -> FormIdentity {
        FormIdentity {
            source_document_id: SourceDocumentId::from("source-a"),
            checked_form_id: CheckedFormId::from("checked-a"),
            expanded_form_id: ExpandedFormId::from("expanded-a"),
        }
    }

    fn candidate(id: &str, status: SoundCandidateStatus) -> SoundCandidateInspection {
        SoundCandidateInspection {
            capability_id: CapabilityId::from(id),
            implementation_id: ImplementationId::from(format!("implementation/{id}")),
            execution_profile_id: ExecutionProfileId::from(format!("profile/{id}")),
            proof_class: SoundProofClass::DeterministicReference,
            status,
            route: SoundRealizationRoute::Direct,
            host_id: None,
            boot_id: None,
            selected_plan_id: None,
        }
    }

    #[test]
    fn explanation_keeps_fit_refusal_route_and_missing_proof_distinct() {
        let inspection = SoundRealizationInspection {
            schema: SOUND_INSPECTION_SCHEMA.into(),
            form: form(),
            requirement_profile_id: "conformance/simple@1".into(),
            candidates: vec![
                candidate("direct", SoundCandidateStatus::Compatible),
                candidate(
                    "mono",
                    SoundCandidateStatus::Incompatible {
                        reason: "polyphony-exceeds-offer".into(),
                    },
                ),
                SoundCandidateInspection {
                    route: SoundRealizationRoute::Recursive {
                        stages: vec![KindId::from("music/synth"), KindId::from("audio/play")],
                    },
                    ..candidate(
                        "physical",
                        SoundCandidateStatus::MissingRequiredProof {
                            required: SoundProofClass::PhysicalPeteCreateHil,
                        },
                    )
                },
            ],
            selected_capability_id: None,
            active_play_id: None,
        };
        let rendered = render_sound_inspection(&inspection).unwrap();
        assert!(rendered.contains("direct | compatible | direct"));
        assert!(rendered.contains("unsupported: polyphony-exceeds-offer"));
        assert!(rendered.contains("music/synth -> audio/play"));
        assert!(rendered.contains("missing proof"));
    }

    #[test]
    fn presentation_controls_cannot_select_an_incompatible_or_unplanned_candidate() {
        let mut incompatible = candidate(
            "candidate",
            SoundCandidateStatus::Incompatible {
                reason: "velocity-unsupported".into(),
            },
        );
        incompatible.host_id = Some(HostId::from("host"));
        incompatible.boot_id = Some(BootId::from("boot"));
        incompatible.selected_plan_id = Some(PlanId::from("plan"));
        let inspection = SoundRealizationInspection {
            schema: SOUND_INSPECTION_SCHEMA.into(),
            form: form(),
            requirement_profile_id: "conformance/expressive@1".into(),
            candidates: vec![incompatible],
            selected_capability_id: Some(CapabilityId::from("candidate")),
            active_play_id: None,
        };
        assert_eq!(
            validate_sound_inspection(&inspection),
            Err(SoundInspectionError::InvalidSelection)
        );
    }
}
