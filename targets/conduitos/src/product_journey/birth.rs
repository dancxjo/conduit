//! Exact initial workset and naming handoff from the shared Crèche.
use super::*;
use conduit_body::{AuthenticatedHostObservation, BodyWorkset, MembershipProofId, ResidentForm};
use conduit_creche_model::birth::BirthSelection;
use sha2::{Digest, Sha256};

impl ProductJourney {
    pub fn birth_from_creche(&mut self, selection: BirthSelection) -> Result<(), JourneyError> {
        self.revision
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        let request_id = format!(
            "conduitos/creche/birth/{}/{}",
            self.boot_id.as_str(),
            selection.revision
        );
        self.birth_workset(
            selection.workset,
            selection.friendly_name,
            self.birth_sequence(),
        )?;
        self.last_request_id = Some(request_id);
        self.advance()
    }

    pub(super) fn birth(&mut self) -> Result<(), JourneyError> {
        let workset = BodyWorkset::one(ResidentForm::new(
            self.form.source_document_id.clone(),
            self.form.checked_form_id.clone(),
        ))
        .map_err(|_| JourneyError::WrongTarget)?;
        self.birth_workset(workset, "My Body".into(), self.birth_sequence())
    }

    fn birth_sequence(&self) -> u64 {
        // Each admitted Boot supplies fresh identity entropy; keep headroom for later biography records.
        let mut hash = Sha256::new();
        hash.update(b"conduitos/creche/birth-sequence@1");
        hash.update(self.host_id.as_str().as_bytes());
        hash.update(self.boot_id.as_str().as_bytes());
        let bytes = hash.finalize();
        u64::from_le_bytes(bytes[..8].try_into().expect("SHA-256 prefix")) >> 1
    }

    fn birth_workset(
        &mut self,
        workset: conduit_body::BodyWorkset,
        name: String,
        sequence: u64,
    ) -> Result<(), JourneyError> {
        if workset.is_empty() || workset.len() > 2 {
            return Err(JourneyError::WrongTarget);
        }
        let mut forms = [None; 2];
        for (slot, resident) in forms.iter_mut().zip(workset.forms()) {
            *slot = Some(native_workset::resolve(resident).map_err(|_| JourneyError::WrongTarget)?);
        }
        let foreground = forms
            .iter()
            .position(|form| *form == Some(NativeForm::KeyboardCanvas))
            .unwrap_or(0);
        let first = native_workset::checked(forms[foreground].ok_or(JourneyError::WrongTarget)?)
            .map_err(JourneyError::Workset)?;
        if name.trim().is_empty()
            || name.len() > conduit_body::MAX_BODY_FRIENDLY_NAME_BYTES
            || name.chars().any(char::is_control)
        {
            return Err(JourneyError::InvalidTransition);
        }
        if self.body.is_some() {
            return Err(JourneyError::AlreadyBorn);
        }
        if self.status != JourneyStatus::FormOpened {
            return Err(JourneyError::FormNotOpened);
        }
        let born_sign = SignId::from(format!(
            "conduitos/product/born/{}/{}",
            self.boot_id.as_str(),
            self.revision
        ));
        let body = Body::born_with_forms(workset, sequence, born_sign.clone())
            .map_err(|_| JourneyError::InvalidTransition)?;
        let part = PartId::bind(&body.body_id, self.host_id.as_str(), 0)
            .map_err(|_| JourneyError::Membership)?;
        let proof = MembershipProofId::bind("conduitos/product/local-birth")
            .map_err(|_| JourneyError::Membership)?;
        let mut membership =
            BodyMembership::new(body.body_id.clone()).map_err(|_| JourneyError::Membership)?;
        membership
            .admit(
                &body.body_id,
                membership.revision,
                part.clone(),
                proof.clone(),
                SignId::from("conduitos/product/part-admitted"),
            )
            .map_err(|_| JourneyError::Membership)?;
        membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: self.host_id.clone(),
                    boot_id: self.boot_id.clone(),
                    offer_generation: self.offer_generation,
                    proof_id: proof,
                    sequence: 0,
                },
                SignId::from("conduitos/product/host-attached"),
            )
            .map_err(|_| JourneyError::Membership)?;
        self.friendly_name = Some(name);
        self.forms = forms;
        self.foreground = foreground;
        self.form = KeyboardTextFormIdentity {
            source_document_id: first.source_document_id,
            checked_form_id: first.checked_form_id,
            expanded_form_id: first.expanded_form_id,
        };
        self.body = Some(body);
        self.born_sign_id = Some(born_sign);
        self.membership = Some(membership);
        self.part_id = Some(part);
        self.status = JourneyStatus::BornLulled;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn journey(boot: &str) -> ProductJourney {
        let mut journey =
            ProductJourney::new(HostId::from("host"), BootId::from(boot), OfferGeneration(1))
                .unwrap();
        journey.open_form().unwrap();
        journey
    }
    fn selection(journey: &ProductJourney) -> BirthSelection {
        BirthSelection {
            revision: 3,
            friendly_name: "Roseau".into(),
            workset: BodyWorkset::one(ResidentForm::new(
                journey.form.source_document_id.clone(),
                journey.form.checked_form_id.clone(),
            ))
            .unwrap(),
        }
    }
    #[test]
    fn creche_birth_keeps_exact_workset_name_and_distinct_new_body_identity() {
        let mut first = journey("boot-a");
        let selection = selection(&first);
        first.birth_from_creche(selection.clone()).unwrap();
        assert_eq!(first.body.as_ref().unwrap().workset, selection.workset);
        assert_eq!(first.projection().friendly_name.as_deref(), Some("Roseau"));
        assert_eq!(first.status(), JourneyStatus::BornLulled);
        assert!(first.plan.is_none() && first.play.is_none());
        let mut second = journey("boot-b");
        second.birth_from_creche(selection.clone()).unwrap();
        assert_ne!(
            first.body.as_ref().unwrap().body_id,
            second.body.as_ref().unwrap().body_id
        );
        assert_eq!(
            first.birth_from_creche(selection),
            Err(JourneyError::AlreadyBorn)
        );
    }
    #[test]
    fn unreviewed_inventory_and_invalid_name_refuse_without_birth() {
        let mut journey = journey("boot");
        let before = journey.projection();
        let mut choice = selection(&journey);
        choice.workset = BodyWorkset::one(ResidentForm::new(
            "source/other".into(),
            "checked/other".into(),
        ))
        .unwrap();
        assert_eq!(
            journey.birth_from_creche(choice),
            Err(JourneyError::WrongTarget)
        );
        let mut choice = selection(&journey);
        choice.friendly_name = "\n".into();
        assert_eq!(
            journey.birth_from_creche(choice),
            Err(JourneyError::InvalidTransition)
        );
        assert_eq!(journey.projection(), before);
    }
}
