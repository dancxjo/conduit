//! Exact transitions from an unbodied Host into one live local body session.

use conduit_body::{
    AdmissionManager, AuthenticatedHostObservation, Body, BodyMembership, CandidateInventory,
    MembershipProofId, PartId, Wake,
};
use conduit_core::SignId;
use conduit_creche_model::birth::BirthSelection;
use std::sync::Arc;

use crate::{
    front_door_topology::FrontDoorTopology, BodyJoinCandidate, FormCandidate, FormEditor,
    LocalFrontDoor, PatchbayModel, RetainedBirthEvidence,
};

impl LocalFrontDoor {
    pub(super) fn join_existing(
        adapter: Arc<dyn crate::PatchbayHostAdapter>,
        model: PatchbayModel,
        candidate: BodyJoinCandidate,
        revision: u64,
    ) -> Result<Self, String> {
        Self::from_existing(
            adapter,
            model,
            Some(candidate.editor),
            candidate.body,
            Some(candidate.wake),
            candidate.membership,
            candidate.proof_id,
            revision,
            "joined",
            None,
        )
    }

    pub(super) fn born_from_form(
        adapter: Arc<dyn crate::PatchbayHostAdapter>,
        model: PatchbayModel,
        form: FormCandidate,
        revision: u64,
    ) -> Result<Self, String> {
        let editor = form.editor()?;
        let checked_form_id = form.checked_form_id.clone();
        let body = Body::born(
            form.source_document_id,
            checked_form_id.clone(),
            revision,
            SignId::from(format!("patchbay/front-door/born/{revision}")),
        )
        .map_err(|error| error.to_string())?;
        let membership =
            BodyMembership::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
        let proof =
            MembershipProofId::bind(&format!("explicit-birth/{}", checked_form_id.as_str()))
                .map_err(|error| error.to_string())?;
        Self::from_existing(
            adapter,
            model,
            Some(editor),
            body,
            None,
            membership,
            proof,
            revision,
            "born",
            None,
        )
    }

    pub(super) fn born_from_selection(
        adapter: Arc<dyn crate::PatchbayHostAdapter>,
        model: PatchbayModel,
        selection: BirthSelection,
        selected: Vec<FormCandidate>,
        revision: u64,
    ) -> Result<Self, String> {
        let friendly_name = selection.friendly_name.trim();
        if selection.revision == 0
            || friendly_name.is_empty()
            || friendly_name.len() > conduit_creche_model::names::MAX_FRIENDLY_NAME_BYTES
            || friendly_name.chars().any(char::is_control)
            || selection.workset.validate().is_err()
            || selected.len() != selection.workset.len()
        {
            return Err("Crèche selection evidence does not match reviewed forms".into());
        }
        let sign_id = SignId::from(format!("patchbay/creche/born/{revision}"));
        let body = Body::born_with_forms(selection.workset.clone(), revision, sign_id.clone())
            .map_err(|error| error.to_string())?;
        let membership =
            BodyMembership::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
        let proof =
            MembershipProofId::bind(&format!("explicit-creche-birth/{}", selection.revision))
                .map_err(|error| error.to_string())?;
        let editor = selected.first().map(FormCandidate::editor).transpose()?;
        let evidence = RetainedBirthEvidence {
            selection_revision: selection.revision,
            friendly_name: friendly_name.into(),
            workset: selection.workset,
            sign_id,
        };
        Self::from_existing(
            adapter,
            model,
            editor,
            body,
            None,
            membership,
            proof,
            revision,
            "born",
            Some(evidence),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_existing(
        adapter: Arc<dyn crate::PatchbayHostAdapter>,
        model: PatchbayModel,
        editor: Option<FormEditor>,
        body: Body,
        wake: Option<Wake>,
        mut membership: BodyMembership,
        proof: MembershipProofId,
        revision: u64,
        transition: &str,
        birth_evidence: Option<RetainedBirthEvidence>,
    ) -> Result<Self, String> {
        let form_name = match (body.workset.forms().first(), editor.as_ref()) {
            (Some(resident), Some(editor)) => Some(
                editor
                    .view()
                    .checked
                    .forms
                    .iter()
                    .find(|form| form.checked_form_id == resident.checked_form_id)
                    .map(|form| form.name.clone())
                    .ok_or("Body checked form is absent from its source document")?,
            ),
            (None, None) => None,
            _ => return Err("Body resident Form and editor presence disagree".into()),
        };
        let here = PartId::bind(
            &body.body_id,
            model.advertisement().host_id.as_str(),
            revision,
        )
        .map_err(|error| error.to_string())?;
        membership
            .admit(
                &body.body_id,
                membership.revision,
                here.clone(),
                proof.clone(),
                SignId::from(format!("patchbay/front-door/{transition}/part/{revision}")),
            )
            .map_err(|error| format!("{error:?}"))?;
        membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &here,
                AuthenticatedHostObservation {
                    host_id: model.advertisement().host_id.clone(),
                    boot_id: model.advertisement().boot_id.clone(),
                    offer_generation: model.advertisement().offer_generation,
                    proof_id: proof,
                    sequence: revision,
                },
                SignId::from(format!("patchbay/front-door/{transition}/host/{revision}")),
            )
            .map_err(|error| format!("{error:?}"))?;
        let candidates =
            CandidateInventory::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
        let admissions =
            AdmissionManager::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
        Ok(Self {
            adapter,
            model,
            editor,
            form_name,
            birth_evidence,
            body,
            wake,
            membership,
            candidates,
            admissions,
            here,
            plan: None,
            play: None,
            active_play: None,
            topology: FrontDoorTopology::default(),
            revision: revision
                .checked_add(1)
                .ok_or("front-door presentation revision exhausted")?,
        })
    }
}
