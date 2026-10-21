//! One retained-state update path. Host effects remain owner requests.
use super::resident_command::{contribution_matches, has_property};
use super::*;
use conduit_body::BodyId;
use conduit_face::{
    Face, FaceContributionBasis, FaceInteractionRefusal, FacePropertyValue, MaskShow,
};

impl PatchbayApplicationPort {
    /// Execute only at the owner's admitted ingress, using its current retained
    /// Face/acknowledged Show and actual contribution basis. No Host effect runs
    /// here: Edit/Mask changes return typed requests to the existing owner.
    pub fn execute_face_command(
        &mut self,
        prepared: PreparedPatchbayCommand,
        body: &BodyId,
        basis: &FaceContributionBasis,
        face: &Face,
        show: &MaskShow,
    ) -> Result<Option<PatchbayApplicationRequest>, PatchbayCommandRefusal> {
        if prepared.body != *body || face.basis.body_id.as_ref() != Some(body) {
            return Err(PatchbayCommandRefusal::WrongOwner);
        }
        if prepared.basis != *basis || !contribution_matches(face, basis) {
            return Err(PatchbayCommandRefusal::InvalidContribution);
        }
        if prepared.revision != self.revision
            || prepared.face_id != face.identity
            || prepared.face_revision != face.revision
            || prepared.mask_mode != self.mask_topology.as_ref().map(|topology| topology.mode)
        {
            return Err(PatchbayCommandRefusal::StaleState);
        }
        if prepared.show_id != show.show_id {
            return Err(PatchbayCommandRefusal::InvalidInteraction(
                FaceInteractionRefusal::StaleShow,
            ));
        }
        prepared
            .interaction
            .validate_against(face, show)
            .map_err(PatchbayCommandRefusal::InvalidInteraction)?;
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
        self.apply_resident_command(prepared.command)
            .map_err(PatchbayCommandRefusal::Domain)
    }

    pub(super) fn apply_event(
        &mut self,
        event: &ApplicationEvent,
    ) -> Result<Option<PatchbayApplicationRequest>, PatchbayApplicationRefusal> {
        if event.action == resident_canvas::SELECT_SUBJECT
            && event.kind == ApplicationEventKind::Change
        {
            self.select_canvas_subject(&event.value)?;
            return Ok(None);
        }
        if event.kind != ApplicationEventKind::Activate {
            return Err(PatchbayApplicationRefusal::UnknownAction);
        }
        let command = match event.action.as_str() {
            INSPECT_NEXT_ACTION_ID => {
                let count = self
                    .selected_active_plot()?
                    .graph
                    .subject_identities()
                    .count();
                if count == 0 {
                    return Err(PatchbayApplicationRefusal::NoSubject);
                }
                let revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(PatchbayApplicationRefusal::RevisionExhausted)?;
                self.selected_subject = (self.selected_subject + 1) % count;
                self.revision = revision;
                return Ok(None);
            }
            EDIT_CURRENT_ACTION_ID => PatchbayResidentCommand::EditSubject {
                expanded_plot_id: self.selected_active_plot()?.graph.expanded_plot_id.clone(),
                subject_identity: self.inspection()?.subject_identity,
            },
            CHANGE_MASKS_ACTION_ID => {
                let topology = self
                    .mask_topology
                    .as_ref()
                    .ok_or(PatchbayApplicationRefusal::UnknownAction)?;
                PatchbayResidentCommand::ChangeMasks {
                    body_plan_id: topology.body_plan_id.clone(),
                    mode: topology.next_mode(),
                }
            }
            action if action.starts_with(SELECT_PLOT_ACTION_PREFIX) => {
                let index = action[SELECT_PLOT_ACTION_PREFIX.len()..]
                    .parse::<usize>()
                    .map_err(|_| PatchbayApplicationRefusal::UnknownAction)?;
                let plot = self
                    .plots
                    .get(index)
                    .ok_or(PatchbayApplicationRefusal::UnknownAction)?;
                PatchbayResidentCommand::SelectPlot {
                    checked_plot_id: plot.graph.checked_plot_id.clone(),
                    plan_id: plot.plan_id.clone(),
                }
            }
            _ => return Err(PatchbayApplicationRefusal::UnknownAction),
        };
        self.apply_resident_command(command)
    }

    fn apply_resident_command(
        &mut self,
        command: PatchbayResidentCommand,
    ) -> Result<Option<PatchbayApplicationRequest>, PatchbayApplicationRefusal> {
        // No state write precedes a fallible check, including revision capacity.
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(PatchbayApplicationRefusal::RevisionExhausted)?;
        let request = match command {
            PatchbayResidentCommand::SelectPlot {
                checked_plot_id,
                plan_id,
            } => {
                let index = self
                    .plots
                    .iter()
                    .position(|plot| {
                        plot.graph.checked_plot_id == checked_plot_id && plot.plan_id == plan_id
                    })
                    .ok_or(PatchbayApplicationRefusal::UnknownAction)?;
                self.selected_plot = Some(index);
                self.selected_subject = 0;
                self.edit_requested = false;
                None
            }
            PatchbayResidentCommand::EditSubject {
                expanded_plot_id,
                subject_identity,
            } => {
                let plot = self.selected_active_plot()?;
                if plot.graph.expanded_plot_id != expanded_plot_id
                    || !plot
                        .graph
                        .subject_identities()
                        .any(|identity| identity == subject_identity)
                {
                    return Err(PatchbayApplicationRefusal::NoSubject);
                }
                self.edit_requested = true;
                Some(PatchbayApplicationRequest::EditCurrent {
                    expanded_plot_id,
                    subject_identity,
                })
            }
            PatchbayResidentCommand::ChangeMasks { body_plan_id, mode } => {
                let topology = self
                    .mask_topology
                    .as_ref()
                    .ok_or(PatchbayApplicationRefusal::UnknownAction)?;
                if topology.body_plan_id != body_plan_id || topology.next_mode() != mode {
                    return Err(PatchbayApplicationRefusal::UnknownAction);
                }
                Some(PatchbayApplicationRequest::ChangeMasks { body_plan_id, mode })
            }
        };
        self.revision = revision;
        Ok(request)
    }
}
