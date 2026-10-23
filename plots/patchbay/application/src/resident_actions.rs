//! Source-owned action contracts; projection does not admit or execute a route.
use super::{
    PatchbayApplicationPort, PatchbayMaskMode, CHANGE_MASKS_ACTION_ID, EDIT_CURRENT_ACTION_ID,
    SELECT_PLOT_ACTION_PREFIX,
};
use alloc::{format, vec, vec::Vec};
use conduit_body::BodyId;
use conduit_face::{
    FaceAction, FaceActionArgument, FaceActionAvailability, FaceContributionBasis,
    FaceDisclosureLevel, FaceFragment, FaceFragmentError, FaceProperty, FacePropertyValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayFaceProjectionError {
    InvalidFragment(FaceFragmentError),
    InvalidActionContract,
}

impl PatchbayApplicationPort {
    /// Prepare effects only after the owner has admitted an exact return route
    /// to this resident Play. This flag is not a grant and comes from the owner,
    /// never from a Mask. False retains explicit unavailable action contracts.
    pub fn face_fragment(
        &self,
        basis: FaceContributionBasis,
        body: &BodyId,
        actions_admitted: bool,
    ) -> Result<FaceFragment, PatchbayFaceProjectionError> {
        let mut fragment = self
            .observe_fragment(basis, body)
            .map_err(PatchbayFaceProjectionError::InvalidFragment)?;
        let root = fragment.subjects[0].identity.clone();
        let availability = if actions_admitted {
            FaceActionAvailability::Available
        } else {
            FaceActionAvailability::Unavailable {
                reason_code: "patchbay-return-route-unavailable".into(),
                explanation: "This Patchbay Play has no admitted action return route.".into(),
            }
        };
        for (index, plot) in self.plots.iter().enumerate() {
            fragment.actions.push(FaceAction {
                identity: format!("{SELECT_PLOT_ACTION_PREFIX}{index}"),
                intent: "patchbay/select-plot@1".into(),
                target: format!("plot/{}", plot.graph.checked_plot_id.as_str()),
                name: format!("Inspect {}", plot.title),
                arguments: Vec::new(),
                disclosure: FaceDisclosureLevel::CurrentAction,
                availability: availability.clone(),
            });
        }
        if let Some(index) = self.selected_plot {
            // Reading focus is Mask-local. Editing therefore names an explicit
            // semantic subject instead of depending on a renderer's cursor.
            let mut argument = FaceActionArgument::text(
                "subject".into(),
                "Subject to edit".into(),
                1,
                conduit_face::MAX_FACE_ID_BYTES as u32,
            )
            .map_err(|_| PatchbayFaceProjectionError::InvalidActionContract)?;
            // These are already the only subjects this owner can edit. Publish
            // their finite membership for Masks which can realize a chooser.
            // Larger contracts retain the typed entry path and owner check.
            let mut members: Vec<Vec<u8>> = self.plots[index]
                .graph
                .subject_identities()
                .map(|identity| identity.as_bytes().to_vec())
                .collect();
            members.sort();
            members.dedup();
            let mut constraints = argument.contract.constraints.clone();
            constraints.push(conduit_core::ValueConstraint::CanonicalMembership {
                members,
                negated: false,
            });
            match conduit_core::CheckedValueContract::new(
                argument.contract.value_kind.clone(),
                argument.contract.maximum_bytes,
                constraints,
            ) {
                Ok(contract) => argument.contract = contract,
                Err(
                    conduit_core::ConstraintDefinitionError::EmptyMembership
                    | conduit_core::ConstraintDefinitionError::TooManyMembershipValues
                    | conduit_core::ConstraintDefinitionError::MembershipBytesExceeded,
                ) => {}
                Err(_) => return Err(PatchbayFaceProjectionError::InvalidActionContract),
            }
            fragment.actions.push(FaceAction {
                identity: EDIT_CURRENT_ACTION_ID.into(),
                intent: "patchbay/edit-subject@1".into(),
                target: format!("plot/{}", self.plots[index].graph.checked_plot_id.as_str()),
                name: "Edit a subject".into(),
                arguments: vec![argument],
                disclosure: FaceDisclosureLevel::CurrentAction,
                availability: availability.clone(),
            });
        }
        if let Some(topology) = &self.mask_topology {
            fragment.properties.extend([
                FaceProperty {
                    subject: root.clone(),
                    name: "mask-mode".into(),
                    value: FacePropertyValue::Text(mode_name(topology.mode).into()),
                },
                FaceProperty {
                    subject: root.clone(),
                    name: "requested-next-mask-mode".into(),
                    value: FacePropertyValue::Text(mode_name(topology.next_mode()).into()),
                },
            ]);
            fragment.actions.push(FaceAction {
                identity: CHANGE_MASKS_ACTION_ID.into(),
                intent: "patchbay/change-masks@1".into(),
                target: root,
                name: match topology.mode {
                    PatchbayMaskMode::Graphical => "Wear speech Mask",
                    PatchbayMaskMode::GraphicalAndSpeech => "Doff graphical Mask",
                    PatchbayMaskMode::Speech => "Wear graphical Mask",
                }
                .into(),
                arguments: Vec::new(),
                disclosure: FaceDisclosureLevel::CurrentAction,
                availability,
            });
        }
        fragment
            .validate_bounds()
            .map_err(PatchbayFaceProjectionError::InvalidFragment)?;
        Ok(fragment)
    }
}

fn mode_name(mode: PatchbayMaskMode) -> &'static str {
    match mode {
        PatchbayMaskMode::Graphical => "graphical",
        PatchbayMaskMode::GraphicalAndSpeech => "graphical-and-speech",
        PatchbayMaskMode::Speech => "speech",
    }
}
