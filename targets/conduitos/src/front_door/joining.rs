//! One semantic Face for a provisioned guest awaiting real owner admission.
use alloc::{format, vec, vec::Vec};
use conduit_birth_plot::BirthFaceBasis;
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationDisclosure, PresentationDisclosureLevel,
    PresentationProperty, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
};

use super::{Error, FrontDoor};
use crate::spore_join::PendingNativeJoin;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingJoinView(PendingNativeJoin);

impl FrontDoor {
    /// The Host-owned pending Face carries the identity of the actual
    /// forwarding Plot when it is published through the native Mask.
    // The freestanding product binary uses this library entry; hosted library
    // builds do not link that binary and cannot see its call site.
    #[allow(dead_code)]
    pub(crate) fn joining_face(&self, producer: &BirthFaceBasis) -> Result<Presentation, Error> {
        self.joining
            .as_ref()
            .ok_or(Error::ActionUnavailable)?
            .presentation(self, Some(producer))
    }

    pub fn await_join(&mut self, pending: PendingNativeJoin) -> Result<(), Error> {
        if self.arrival.is_some()
            || self.joining.is_some()
            || self
                .journey
                .as_ref()
                .is_some_and(|journey| journey.body_id.is_some())
        {
            return Err(Error::ActionUnavailable);
        }
        self.lifecycle_authority_admitted = false;
        self.joining = Some(PendingJoinView(pending));
        self.advance()
    }

    pub const fn joining_pending(&self) -> bool {
        self.joining.is_some()
    }
}

impl PendingJoinView {
    pub(super) fn presentation(
        &self,
        door: &FrontDoor,
        producer: Option<&BirthFaceBasis>,
    ) -> Result<Presentation, Error> {
        let host = format!("host/{}/{}", door.host_id.as_str(), door.boot_id.as_str());
        let candidate = format!("candidate/{}", self.0.spore_id);
        Presentation::new_with_semantics(
            door.revision,
            PresentationBasis {
                body_id: None,
                wake_id: None,
                source_document_id: producer.map(|basis| basis.producer_plot.source_document_id.clone()),
                checked_plot_id: producer.map(|basis| basis.producer_plot.checked_plot_id.clone()),
                expanded_plot_id: producer.map(|basis| basis.producer_plot.expanded_plot_id.clone()),
                plan_id: producer.map(|basis| basis.producer_plan_id.clone()),
                active_play_id: None,
                sign_ids: Vec::new(),
            },
            vec![
                PresentationSubject {
                    identity: host.clone(),
                    role: PresentationRole::Host,
                    name: "This ConduitOS host".into(),
                },
                PresentationSubject {
                    identity: candidate.clone(),
                    role: PresentationRole::Candidate,
                    name: "Invitation to another Body".into(),
                },
            ],
            vec![PresentationRelationship {
                source: host.clone(),
                target: candidate.clone(),
                kind: PresentationRelationshipKind::Observes,
            }],
            vec![
                property(&host, "current-body", PresentationPropertyValue::Text("none".into())),
                property(&candidate, "target-body-id", PresentationPropertyValue::Identity(self.0.body_id.clone())),
                property(&candidate, "invitation-id", PresentationPropertyValue::Identity(self.0.invitation_id.clone())),
                property(&candidate, "spore-id", PresentationPropertyValue::Identity(self.0.spore_id.clone())),
                property(&candidate, "owner-route-candidates", PresentationPropertyValue::Count(
                    self.0.rendezvous.as_ref().map_or(0, |route| route.candidates.len()) as u64,
                )),
                property(&candidate, "membership-admitted", PresentationPropertyValue::Flag(false)),
            ],
            vec![
                PresentationText {
                    subject: host.clone(),
                    text: "This host has sent an invitation-bound request. It has not joined a Body, and no Plan or Play is active here.".into(),
                },
                PresentationText {
                    subject: candidate.clone(),
                    text: if self.0.rendezvous.is_some() {
                        "A candidate route to the owner is recorded, but no connection or admission receipt has completed. Local Birth is unavailable; this guest will not create a different Body."
                    } else {
                        "Waiting for an authenticated owner route and admission receipt. Local Birth is unavailable; this guest will not create a different Body."
                    }.into(),
                },
            ],
            Vec::new(),
            vec![
                PresentationDisclosure {
                    subject: candidate,
                    level: PresentationDisclosureLevel::Primary,
                },
                PresentationDisclosure {
                    subject: host,
                    level: PresentationDisclosureLevel::Context,
                },
            ],
        )
        .map_err(|_| Error::Presentation)
    }
}

fn property(subject: &str, name: &str, value: PresentationPropertyValue) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        BootId, CheckedPlotId, ExpandedPlotId, HostId, OfferGeneration, PlanId, PlotIdentity,
        SourceDocumentId,
    };

    #[test]
    fn provisioned_guest_exposes_pending_candidate_without_a_body_or_birth() {
        let mut door = FrontDoor::new(
            HostId::from("host/native"),
            BootId::from("boot/native"),
            OfferGeneration(1),
            "profile/native",
            "build/native",
            "image/native",
            SourceDocumentId::from("source/native"),
            CheckedPlotId::from("checked/native"),
            1,
            true,
        );
        door.await_join(PendingNativeJoin {
            spore_id: "spore/one".into(),
            body_id: "body/invited".into(),
            invitation_id: "invitation/one".into(),
            rendezvous: None,
        })
        .unwrap();
        assert!(door.joining_pending());
        let face = door.presentation().unwrap();
        face.validate().unwrap();
        assert!(face.basis.body_id.is_none());
        assert!(face.actions.is_empty());
        assert!(face.properties.iter().any(|property| {
            property.name == "membership-admitted"
                && property.value == PresentationPropertyValue::Flag(false)
        }));
        assert!(
            face.text
                .iter()
                .any(|text| text.text.contains("Local Birth is unavailable"))
        );
        assert!(door.open_creche("new-body".into(), None).is_err());
        let published = door
            .joining_face(&BirthFaceBasis {
                host_id: HostId::from("host/native"),
                boot_id: BootId::from("boot/native"),
                encounter_id: "joining/one".into(),
                producer_plot: PlotIdentity {
                    source_document_id: SourceDocumentId::from("producer/source"),
                    checked_plot_id: CheckedPlotId::from("producer/checked"),
                    expanded_plot_id: ExpandedPlotId::from("producer/expanded"),
                },
                producer_plan_id: PlanId::from("producer/plan"),
            })
            .unwrap();
        assert_eq!(published.basis.plan_id, Some(PlanId::from("producer/plan")));
        assert!(published.basis.body_id.is_none());
    }

    #[test]
    fn provisioned_owner_route_remains_visible_without_claiming_a_connection() {
        let route = serde_json::from_value(serde_json::json!({
            "protocol": 1,
            "body_id": "body/invited",
            "invitation_id": "invitation/one",
            "candidates": [{
                "candidate_id": "candidate/owner",
                "line_family": "authenticated-tls-stream",
                "reachability": "wss://owner.example:443/conduit",
                "authentication": {
                    "server_identity": "owner.example",
                    "transport_binding_sha256": vec![7; 32]
                },
                "expires_at_millis": 1_800_000_000_000_u64,
                "maximum_attempts": 1,
                "attempt_timeout_millis": 2_000
            }]
        }))
        .unwrap();
        let mut door = FrontDoor::new(
            HostId::from("host/native"),
            BootId::from("boot/native"),
            OfferGeneration(1),
            "profile/native",
            "build/native",
            "image/native",
            SourceDocumentId::from("source/native"),
            CheckedPlotId::from("checked/native"),
            1,
            true,
        );
        door.await_join(PendingNativeJoin {
            spore_id: "spore/one".into(),
            body_id: "body/invited".into(),
            invitation_id: "invitation/one".into(),
            rendezvous: Some(route),
        })
        .unwrap();
        let face = door.presentation().unwrap();
        face.validate().unwrap();
        assert!(face.basis.body_id.is_none());
        assert!(face.properties.iter().any(|property| {
            property.name == "owner-route-candidates"
                && property.value == PresentationPropertyValue::Count(1)
        }));
        assert!(face.text.iter().any(|text| {
            text.text
                .contains("no connection or admission receipt has completed")
        }));
    }
}
