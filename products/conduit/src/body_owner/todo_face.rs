//! Exact Todo contribution to the installed owner's Face. The caller must
//! supply state recovered through the admitted Play; this module stores none.

use super::{clock_interval, Owner};
use conduit_presentation::{
    CommittedFaceAdmission, CommittedStateContributionBasis, CommittedStateEvidenceRefusal,
    CommittedStateSelection, Face, FaceContext, FaceContribution, FaceContributionRole, FaceFocus,
    FaceInteraction, FaceNames, FaceResidentPlotName, MaskShow, Presentation,
    PresentationContributionBasis,
};
use conduit_todo_face::{todo_command_from_contributed_interaction, todo_fragment};
use conduit_todo_plot::{TodoCommand, TodoState};
use sha2::{Digest, Sha256};

impl Owner {
    pub(super) fn project_verified_todo_face(
        &self,
        basis: &CommittedStateContributionBasis,
        state: &TodoState,
    ) -> Result<Presentation, String> {
        let receipt = self
            .todo_verified_read_receipt()
            .ok_or(CommittedStateEvidenceRefusal::MissingReadReceipt.as_str())?;
        if !receipt["read_terminal_sign"]["sign_id"].is_string()
            || !receipt["write"]["terminal_sign"]["sign_id"].is_string()
        {
            return Err(CommittedStateEvidenceRefusal::MissingTerminalSign
                .as_str()
                .into());
        }
        if receipt["read_terminal"] != "Completed"
            || !receipt["read_failure"].is_null()
            || !receipt["read_cleanup_failure"].is_null()
            || !receipt["read_kernel_failure"].is_null()
            || receipt["write"]["terminal"] != "Completed"
            || !receipt["write"]["failure"].is_null()
            || !receipt["write"]["cleanup_failure"].is_null()
        {
            return Err(CommittedStateEvidenceRefusal::FailedTerminalSign
                .as_str()
                .into());
        }
        let encoded = state.encode_info().map_err(super::debug)?;
        let mut digest = [0; 32];
        digest.copy_from_slice(&Sha256::digest(&encoded));
        let expected_sha = format!("sha256:{:x}", Sha256::digest(&encoded));
        if basis.state_digest != digest || receipt["restored_fore_sha256"] != expected_sha {
            return Err(CommittedStateEvidenceRefusal::ReadDigestMismatch
                .as_str()
                .into());
        }
        if receipt["body_id"] != basis.body_id.as_str()
            || receipt["read_plan_id"] != basis.read.plan_id.as_str()
            || receipt["read_play"]["active_play_id"] != basis.read.play_id.as_str()
            || receipt["read_terminal_sign"]["sign_id"] != basis.read.terminal_sign_id.as_str()
            || receipt["read_terminal_sign"]["active_play_id"] != basis.read.play_id.as_str()
            || receipt["write"]["plan_id"] != basis.write.plan_id.as_str()
            || receipt["write"]["play"]["active_play_id"] != basis.write.play_id.as_str()
            || receipt["write"]["terminal_sign"]["sign_id"] != basis.write.terminal_sign_id.as_str()
            || receipt["write"]["terminal_sign"]["active_play_id"] != basis.write.play_id.as_str()
        {
            return Err(CommittedStateEvidenceRefusal::EvidenceMismatch
                .as_str()
                .into());
        }
        let advertised = self.host.advertisement();
        if receipt["read_terminal_sign"]["host_id"] != advertised.host_id.as_str()
            || receipt["read_terminal_sign"]["boot_id"] != advertised.boot_id.as_str()
        {
            return Err(CommittedStateEvidenceRefusal::BootChanged.as_str().into());
        }
        let content = selected_read_residence(advertised)?;
        if receipt["selected_content"] != serde_json::json!(content.contract) {
            return Err(CommittedStateEvidenceRefusal::ReadVersionMismatch
                .as_str()
                .into());
        }
        if receipt["selected_residence"] != serde_json::json!(content) {
            return Err(CommittedStateEvidenceRefusal::ReadResidenceChanged
                .as_str()
                .into());
        }
        let current_selection = CommittedStateSelection {
            resource: content.contract.identity,
            selected_version: content.contract.version,
            published_version: content.contract.version,
        };
        let resident = self
            .resident
            .as_ref()
            .ok_or("Todo has no resident read Plot")?;
        let name = self
            .resident_name
            .as_deref()
            .ok_or("Todo has no read Plot name")?;
        if name != "todo/checkpoint-restore" || resident.checked_plot_id != basis.checked_plot_id {
            return Err("Todo verified Face has a different resident Plot".into());
        }
        let fragment = todo_fragment(
            state,
            PresentationContributionBasis {
                checked_plot_id: basis.checked_plot_id.clone(),
                plan_id: basis.read.plan_id.clone(),
                active_play_id: basis.read.play_id.clone(),
                required_interaction_context: None,
            },
            true,
        )
        .map_err(|error| format!("Todo committed contribution refused: {error:?}"))?;
        let names = [FaceResidentPlotName {
            source_document_id: &resident.source_document_id,
            checked_plot_id: &resident.checked_plot_id,
            name,
        }];
        let face = Face::project_committed(
            &self.session.evidence().body,
            self.session.evidence().last_sequence(),
            FaceContext::Overview,
            FaceFocus::Body,
            FaceContribution::from_presentation(FaceContributionRole::Foreground, fragment),
            FaceNames {
                body_name: Some(&self.session.evidence().friendly_name),
                resident_plots: &names,
            },
            CommittedFaceAdmission {
                basis,
                current_selection: &current_selection,
            },
        )
        .map_err(|error| format!("owner-committed-face-refused:{error:?}"))?;
        face.presentation
            .validate()
            .map_err(|error| format!("owner-committed-face-invalid:{error:?}"))?;
        Ok(face.presentation)
    }

    /// Validate a return against the exact verified committed Face before
    /// the owner retires its Mask and admits the next ordinary Todo Play.
    pub(crate) fn resolve_committed_todo_interaction(
        &self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<(TodoState, TodoCommand), String> {
        let (basis, state) = self
            .todo_verified
            .as_ref()
            .ok_or("Todo action has no verified committed state")?;
        let face = self.project_verified_todo_face(basis, state)?;
        let command = todo_command_from_contributed_interaction(
            state,
            &face,
            show,
            interaction,
            PresentationContributionBasis {
                checked_plot_id: basis.checked_plot_id.clone(),
                plan_id: basis.read.plan_id.clone(),
                active_play_id: basis.read.play_id.clone(),
                required_interaction_context: None,
            },
        )
        .map_err(|error| format!("committed Todo action refused: {error:?}"))?;
        Ok((state.clone(), command))
    }

    /// Resolve only an action offered by this exact current Owner Face and
    /// acknowledged Show. The caller retains the state from the admitted Fore
    /// and submits the returned command to that same waiting Play.
    #[allow(dead_code)] // Installed return uses this after the waiting Todo Play is wired.
    pub(crate) fn resolve_todo_interaction(
        &self,
        state: &TodoState,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<TodoCommand, String> {
        let face = self.project_face(Some((state, true)))?;
        let basis = self.todo_basis()?;
        todo_command_from_contributed_interaction(state, &face, show, interaction, basis)
            .map_err(|error| format!("current Todo action refused: {error:?}"))
    }

    fn todo_basis(&self) -> Result<PresentationContributionBasis, String> {
        if !matches!(
            self.resident_name.as_deref(),
            Some("todo/main" | "todo/checkpoint-once" | "todo/checkpoint-restore")
        ) {
            return Err("current resident Plot is not Todo".into());
        }
        let resident = self.resident.as_ref().ok_or("Todo has no resident Plot")?;
        let realization = self
            .session
            .realization()
            .ok_or("Todo has no current Body Plan")?;
        let play = realization
            .play
            .as_ref()
            .ok_or("Todo has no current admitted Play")?;
        realization
            .plan
            .plots
            .iter()
            .find(|entry| entry.plot == *resident)
            .ok_or("Todo resident Plot is not in the current Plan")?;
        Ok(PresentationContributionBasis {
            checked_plot_id: resident.checked_plot_id.clone(),
            plan_id: realization.plan.plan_id.clone(),
            active_play_id: play.active_play_id.clone(),
            required_interaction_context: None,
        })
    }

    /// The ordinary owner Face, optionally including current Todo Play truth.
    /// A Plot contribution cannot be projected from an idle or retired Play.
    pub(super) fn project_face(
        &self,
        todo: Option<(&TodoState, bool)>,
    ) -> Result<Presentation, String> {
        let plot_name = self
            .resident
            .as_ref()
            .zip(self.resident_name.as_deref())
            .map(|(resident, name)| FaceResidentPlotName {
                source_document_id: &resident.source_document_id,
                checked_plot_id: &resident.checked_plot_id,
                name,
            });
        let plot_names: Vec<_> = plot_name.into_iter().collect();
        let contributions = todo
            .map(|(state, actions_admitted)| {
                let fragment = todo_fragment(state, self.todo_basis()?, actions_admitted)
                    .map_err(|error| format!("Todo Face contribution refused: {error:?}"))?;
                Ok::<_, String>(vec![FaceContribution::from_presentation(
                    FaceContributionRole::Foreground,
                    fragment,
                )])
            })
            .transpose()?
            .unwrap_or_default();
        let face = Face::project_with_names(
            &self.session.evidence().body,
            self.session
                .realization()
                .map(|realization| &realization.wake),
            self.session.evidence().last_sequence(),
            FaceContext::Overview,
            FaceFocus::Body,
            contributions,
            FaceNames {
                body_name: Some(&self.session.evidence().friendly_name),
                resident_plots: &plot_names,
            },
        )
        .map_err(|error| format!("owner-face-projection-refused:{error:?}"))?;
        face.presentation
            .validate()
            .map_err(|error| format!("owner-face-invalid:{error:?}"))?;
        clock_interval::with_clock_action(self, face.presentation)
    }
}

/// Revalidate the admitted resource residence and exact read Back independently
/// of semantic content/version. A matching resource contract grants no authority.
pub(super) fn selected_read_residence(
    advertised: &conduit_core::HostAdvertisement,
) -> Result<&conduit_core::ResourceContentOffer, String> {
    let mut selected = advertised.resources.iter().filter_map(|resource| {
        (resource.class_id.as_str() == "resource/todo-checkpoint@1")
            .then_some(resource.content.as_ref())
            .flatten()
    });
    let content = selected
        .next()
        .ok_or(CommittedStateEvidenceRefusal::ReadResidenceChanged.as_str())?;
    if selected.next().is_some()
        || content.contract.access != conduit_core::ResourceAccessMode::ReadPublished
        || content.validate().is_err()
        || content.owner_host != advertised.host_id
        || content.owner_boot != advertised.boot_id
        || content.base_id.as_str() != "std/explicit-shared-checkpoint"
        || content.residence_profile.as_str() != "std/explicit-shared-checkpoint@1"
    {
        return Err(CommittedStateEvidenceRefusal::ReadResidenceChanged
            .as_str()
            .into());
    }
    let expected = conduit_std_offers::todo_checkpoint_read_offer(content.contract.clone())
        .map_err(|_| CommittedStateEvidenceRefusal::ReadAuthorityChanged.as_str())?;
    let mut reads = advertised.capabilities.iter().filter(|offer| {
        offer.capability_id == expected.capability_id
            || offer.implementation.implementation_id == expected.implementation.implementation_id
    });
    if reads.next() != Some(&expected) || reads.next().is_some() {
        return Err(CommittedStateEvidenceRefusal::ReadAuthorityChanged
            .as_str()
            .into());
    }
    Ok(content)
}
