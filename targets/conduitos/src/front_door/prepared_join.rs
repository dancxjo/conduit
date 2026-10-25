//! Retained construction is a candidate for ordinary join, never an invitation.
//!
//! The current fixed boot slot carries retained birth OR an invitation. A
//! future combined envelope must independently validate both bounded variants,
//! match their Body identities, and redeem only the ordinary invitation; this
//! view provides neither invitation acquisition nor an admission shortcut.
use super::*;
use conduit_birth_plot::BirthFaceBasis;
use conduit_body::{PreparedBirthHostRole, PreparedBirthMedia};

impl FrontDoor {
    pub fn await_prepared_join(&mut self, media: PreparedBirthMedia) -> Result<(), Error> {
        if self.arrival.is_some()
            || self.joining.is_some()
            || self.prepared_join.is_some()
            || self
                .journey
                .as_ref()
                .is_some_and(|journey| journey.body_id.is_some())
            || media.role() != PreparedBirthHostRole::LaterHost
            || media.binding().profile_id != self.profile_id
            || media.binding().build_id != self.build_id
        {
            return Err(Error::ActionUnavailable);
        }
        self.revision.checked_add(1).ok_or(Error::Face)?;
        self.lifecycle_authority_admitted = false;
        self.prepared_join = Some(media);
        self.advance()
    }

    pub const fn prepared_join_pending(&self) -> bool {
        self.prepared_join.is_some()
    }

    pub(super) fn prepared_join_face(
        &self,
        producer: Option<&BirthFaceBasis>,
    ) -> Result<Face, Error> {
        let media = self
            .prepared_join
            .as_ref()
            .ok_or(Error::ActionUnavailable)?;
        let host = format!("host/{}/{}", self.host_id.as_str(), self.boot_id.as_str());
        let candidate = format!("candidate/prepared/{}", media.evidence().body_id.as_str());
        let property = |name: &str, value| FaceProperty {
            subject: candidate.clone(),
            name: name.into(),
            value,
        };
        Face::new_with_semantics(
            self.revision,
            FaceBasis {
                body_id: None, wake_id: None,
                source_document_id: producer.map(|basis| basis.producer_plot.source_document_id.clone()),
                checked_plot_id: producer.map(|basis| basis.producer_plot.checked_plot_id.clone()),
                expanded_plot_id: producer.map(|basis| basis.producer_plot.expanded_plot_id.clone()),
                plan_id: producer.map(|basis| basis.producer_plan_id.clone()),
                active_play_id: None, sign_ids: alloc::vec::Vec::new(),
            },
            vec![
                FaceSubject { identity: host.clone(), role: FaceRole::Host, name: "This ConduitOS host".into() },
                FaceSubject { identity: candidate.clone(), role: FaceRole::Candidate, name: media.evidence().friendly_name.clone() },
            ],
            vec![FaceRelationship { source: host.clone(), target: candidate.clone(), kind: FaceRelationshipKind::Observes }],
            vec![
                property("target-body-id", FacePropertyValue::Identity(media.evidence().body_id.as_str().into())),
                property("host-entry", FacePropertyValue::Text(media.binding().host_entry.clone())),
                property("first-host-entry", FacePropertyValue::Text(media.binding().first_host_entry.clone())),
                property("birth-evidence-retained", FacePropertyValue::Flag(true)),
                property("invitation-available", FacePropertyValue::Flag(false)),
                property("membership-admitted", FacePropertyValue::Flag(false)),
                property("owner-receipt-verified", FacePropertyValue::Flag(false)),
            ],
            vec![FaceText { subject: candidate.clone(), text: "This image retains the existing Body's birth. Start its first Host, then supply an ordinary owner invitation for this Host. No invitation or membership has been supplied; local Birth, Wake, Plan and Play are unavailable.".into() }],
            alloc::vec::Vec::new(),
            vec![FaceDisclosure { subject: candidate, level: FaceDisclosureLevel::Primary }, FaceDisclosure { subject: host, level: FaceDisclosureLevel::Context }],
        ).map_err(|_| Error::Face)
    }
}

#[cfg(test)]
mod tests;
