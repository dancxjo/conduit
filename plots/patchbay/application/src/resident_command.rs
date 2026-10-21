//! Pre-effect canonical command admission. The owner supplies retained evidence.
use super::{
    PatchbayApplicationPort, PatchbayFaceProjectionError, PatchbayMaskMode, CHANGE_MASKS_ACTION_ID,
    EDIT_CURRENT_ACTION_ID, SELECT_PLOT_ACTION_PREFIX,
};
use alloc::{format, string::String};
use conduit_body::BodyId;
use conduit_core::{CheckedPlotId, ExpandedPlotId, PlanId};
use conduit_face::{
    Face, FaceContentId, FaceContributionBasis, FaceInteraction, FaceInteractionId,
    FaceInteractionRefusal, FacePropertyValue, MaskShow, MaskShowId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayResidentCommand {
    SelectPlot {
        checked_plot_id: CheckedPlotId,
        plan_id: PlanId,
    },
    EditSubject {
        expanded_plot_id: ExpandedPlotId,
        subject_identity: String,
    },
    ChangeMasks {
        body_plan_id: PlanId,
        mode: PatchbayMaskMode,
    },
}

/// Prepared request, not a grant or an executed effect. State revision and exact
/// owner provenance must be checked again at the admitted execution boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedPatchbayCommand {
    pub(super) revision: u32,
    pub(super) face_id: FaceContentId,
    pub(super) face_revision: u64,
    pub(super) show_id: MaskShowId,
    pub(super) interaction_id: FaceInteractionId,
    pub(super) sequence: u64,
    pub(super) body: BodyId,
    pub(super) basis: FaceContributionBasis,
    pub(super) command: PatchbayResidentCommand,
}
impl PreparedPatchbayCommand {
    pub fn revision(&self) -> u32 {
        self.revision
    }
    pub fn face_id(&self) -> &FaceContentId {
        &self.face_id
    }
    pub fn show_id(&self) -> &MaskShowId {
        &self.show_id
    }
    pub fn interaction_id(&self) -> &FaceInteractionId {
        &self.interaction_id
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn command(&self) -> &PatchbayResidentCommand {
        &self.command
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayCommandRefusal {
    WrongOwner,
    StaleState,
    InvalidContribution,
    InvalidInteraction(FaceInteractionRefusal),
    InvalidActionContract,
    UnknownAction,
    UnknownSubject,
    Projection(PatchbayFaceProjectionError),
}

impl PatchbayApplicationPort {
    /// The owner supplies its current retained Face, actual acknowledged Show
    /// and Playing contribution basis, never evidence supplied by the Mask.
    /// This prepares a typed request only; it neither mutates nor dispatches it.
    pub fn admit_face_command(
        &self,
        body: &BodyId,
        basis: FaceContributionBasis,
        face: &Face,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<PreparedPatchbayCommand, PatchbayCommandRefusal> {
        if face.basis.body_id.as_ref() != Some(body) {
            return Err(PatchbayCommandRefusal::WrongOwner);
        }
        if basis
            .required_interaction_context
            .as_ref()
            .is_some_and(|required| face.interaction_context.identity != *required)
            || !contribution_matches(face, &basis)
        {
            return Err(PatchbayCommandRefusal::InvalidContribution);
        }
        let root = format!("patchbay/document/{}", basis.checked_plot_id.as_str());
        if !has_property(
            face,
            &root,
            "application-revision",
            &FacePropertyValue::Count(u64::from(self.revision)),
        ) || !has_property(
            face,
            &root,
            "body-plan-id",
            &FacePropertyValue::Identity(self.body_plan_id.as_str().into()),
        ) {
            return Err(PatchbayCommandRefusal::StaleState);
        }
        interaction
            .validate_against(face, show)
            .map_err(PatchbayCommandRefusal::InvalidInteraction)?;
        let expected = self
            .face_fragment(basis.clone(), body, true)
            .map_err(PatchbayCommandRefusal::Projection)?;
        let canonical = expected
            .actions
            .iter()
            .find(|action| action.identity == interaction.action_id)
            .ok_or(PatchbayCommandRefusal::UnknownAction)?;
        if face
            .actions
            .iter()
            .find(|action| action.identity == interaction.action_id)
            != Some(canonical)
        {
            return Err(PatchbayCommandRefusal::InvalidActionContract);
        }
        let command = if let Some(index) = interaction
            .action_id
            .strip_prefix(SELECT_PLOT_ACTION_PREFIX)
        {
            let plot = index
                .parse::<usize>()
                .ok()
                .and_then(|index| self.plots.get(index))
                .ok_or(PatchbayCommandRefusal::UnknownAction)?;
            PatchbayResidentCommand::SelectPlot {
                checked_plot_id: plot.graph.checked_plot_id.clone(),
                plan_id: plot.plan_id.clone(),
            }
        } else if interaction.action_id == EDIT_CURRENT_ACTION_ID {
            let argument = interaction
                .arguments
                .iter()
                .find(|argument| argument.name == "subject")
                .ok_or(PatchbayCommandRefusal::InvalidActionContract)?;
            let identity = core::str::from_utf8(&argument.value)
                .map_err(|_| PatchbayCommandRefusal::UnknownSubject)?;
            let plot = self
                .selected_plot
                .and_then(|index| self.plots.get(index))
                .ok_or(PatchbayCommandRefusal::UnknownSubject)?;
            if !plot
                .graph
                .subject_identities()
                .any(|subject| subject == identity)
            {
                return Err(PatchbayCommandRefusal::UnknownSubject);
            }
            PatchbayResidentCommand::EditSubject {
                expanded_plot_id: plot.graph.expanded_plot_id.clone(),
                subject_identity: identity.into(),
            }
        } else if interaction.action_id == CHANGE_MASKS_ACTION_ID {
            let topology = self
                .mask_topology
                .as_ref()
                .ok_or(PatchbayCommandRefusal::UnknownAction)?;
            PatchbayResidentCommand::ChangeMasks {
                body_plan_id: topology.body_plan_id.clone(),
                mode: topology.next_mode(),
            }
        } else {
            return Err(PatchbayCommandRefusal::UnknownAction);
        };
        Ok(PreparedPatchbayCommand {
            revision: self.revision,
            face_id: face.identity.clone(),
            face_revision: face.revision,
            show_id: show.show_id.clone(),
            interaction_id: interaction.identity.clone(),
            sequence: interaction.sequence,
            body: body.clone(),
            basis,
            command,
        })
    }
}

fn has_property(face: &Face, subject: &str, name: &str, value: &FacePropertyValue) -> bool {
    face.properties.iter().any(|property| {
        property.subject == subject && property.name == name && property.value == *value
    })
}

fn contribution_matches(face: &Face, basis: &FaceContributionBasis) -> bool {
    let standalone = face.basis.checked_plot_id.as_ref() == Some(&basis.checked_plot_id)
        && face.basis.plan_id.as_ref() == Some(&basis.plan_id)
        && face.basis.active_play_id.as_ref() == Some(&basis.active_play_id);
    standalone
        || face.properties.iter().any(|property| {
            property.subject.starts_with("contribution/foreground/")
                && property.name == "checked-plot-id"
                && property.value
                    == FacePropertyValue::Identity(basis.checked_plot_id.as_str().into())
                && has_property(
                    face,
                    &property.subject,
                    "plan-id",
                    &FacePropertyValue::Identity(basis.plan_id.as_str().into()),
                )
                && has_property(
                    face,
                    &property.subject,
                    "active-play-id",
                    &FacePropertyValue::Identity(basis.active_play_id.as_str().into()),
                )
        })
}
