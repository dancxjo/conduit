//! One semantic Face for a provisioned guest awaiting real owner admission.
use alloc::{format, vec, vec::Vec};
use conduit_birth_plot::BirthFaceBasis;
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationDisclosure, PresentationDisclosureLevel,
    PresentationProperty, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
};

use super::{Error, FrontDoor};
use crate::spore_join::{OwnerExchange, PendingNativeJoin};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingJoinView(
    PendingNativeJoin,
    Option<crate::native_guest_part::NativeGuestPart>,
);

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
        self.joining = Some(PendingJoinView(pending, None));
        self.advance()
    }

    /// Replace the pending candidate with the owner's exact admitted Part.
    /// This still grants no local lifecycle or workset control.
    pub fn install_join(
        &mut self,
        part: crate::native_guest_part::NativeGuestPart,
    ) -> Result<(), Error> {
        let joining = self.joining.as_mut().ok_or(Error::ActionUnavailable)?;
        if joining.1.is_some()
            || joining.0.body_id != part.credential().body_id.as_str()
            || part.credential().host_id != self.host_id
            || part.credential().boot_id != self.boot_id
            || self
                .journey
                .as_ref()
                .is_some_and(|journey| journey.body_id.is_some())
        {
            return Err(Error::Presentation);
        }
        joining.1 = Some(part);
        self.advance()
    }

    pub const fn joining_pending(&self) -> bool {
        matches!(&self.joining, Some(joining) if joining.1.is_none())
    }
}

impl PendingJoinView {
    pub(super) fn presentation(
        &self,
        door: &FrontDoor,
        producer: Option<&BirthFaceBasis>,
    ) -> Result<Presentation, Error> {
        if let Some(part) = &self.1 {
            return self.admitted_presentation(door, producer, part);
        }
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
                property(&candidate, "owner-receipt-verified", PresentationPropertyValue::Flag(
                    self.0.owner_exchange == OwnerExchange::ReceiptVerified,
                )),
            ],
            vec![
                PresentationText {
                    subject: host.clone(),
                    text: "This host prepared an invitation-bound admission request. It has not joined a Body, and no Plan or Play is active here.".into(),
                },
                PresentationText {
                    subject: candidate.clone(),
                    text: match self.0.owner_exchange {
                        OwnerExchange::ReceiptVerified => "The owner returned an exact verified admission receipt. This guest has not installed membership yet. Local Birth is unavailable; it will not create a different Body.".into(),
                        OwnerExchange::Refused(reason) => format!("Owner admission did not complete ({reason}). This guest remains unjoined. Local Birth is unavailable; it will not create a different Body."),
                        OwnerExchange::NotAttempted if self.0.rendezvous.is_some() => "A candidate route to the owner is recorded, but no connection or admission receipt has completed. Local Birth is unavailable; this guest will not create a different Body.".into(),
                        OwnerExchange::NotAttempted => "Waiting for an authenticated owner route and admission receipt. Local Birth is unavailable; this guest will not create a different Body.".into(),
                    },
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

    fn admitted_presentation(
        &self,
        door: &FrontDoor,
        producer: Option<&BirthFaceBasis>,
        admitted: &crate::native_guest_part::NativeGuestPart,
    ) -> Result<Presentation, Error> {
        let host = format!("host/{}/{}", door.host_id.as_str(), door.boot_id.as_str());
        let credential = admitted.credential();
        let body = format!("body/{}", credential.body_id.as_str());
        let part = format!("part/{}", credential.part_id.as_str());
        Presentation::new_with_semantics(
            door.revision,
            PresentationBasis {
                body_id: Some(credential.body_id.clone()),
                wake_id: None,
                source_document_id: producer.map(|basis| basis.producer_plot.source_document_id.clone()),
                checked_plot_id: producer.map(|basis| basis.producer_plot.checked_plot_id.clone()),
                expanded_plot_id: producer.map(|basis| basis.producer_plot.expanded_plot_id.clone()),
                plan_id: producer.map(|basis| basis.producer_plan_id.clone()),
                active_play_id: None,
                sign_ids: Vec::new(),
            },
            vec![
                PresentationSubject { identity: host.clone(), role: PresentationRole::Host, name: "This ConduitOS host".into() },
                PresentationSubject { identity: body.clone(), role: PresentationRole::Body, name: "Joined Body".into() },
                PresentationSubject { identity: part.clone(), role: PresentationRole::Part, name: "This host's admitted Part".into() },
            ],
            vec![
                PresentationRelationship { source: body.clone(), target: part.clone(), kind: PresentationRelationshipKind::Contains },
                PresentationRelationship { source: part.clone(), target: host.clone(), kind: PresentationRelationshipKind::Contains },
            ],
            vec![
                property(&body, "body-id", PresentationPropertyValue::Identity(credential.body_id.as_str().into())),
                property(&part, "part-id", PresentationPropertyValue::Identity(credential.part_id.as_str().into())),
                property(&part, "credential-id", PresentationPropertyValue::Identity(credential.credential_id.as_str().into())),
                property(&part, "membership-admitted", PresentationPropertyValue::Flag(true)),
                property(&part, "present-at-admission", PresentationPropertyValue::Flag(admitted.is_present())),
                property(&body, "workset-known-here", PresentationPropertyValue::Flag(false)),
            ],
            vec![
                PresentationText { subject: body.clone(), text: "The owner admitted this host to this Body. Its workset and current lifecycle have not yet been transferred here.".into() },
                PresentationText { subject: part.clone(), text: "This Part was present on this Host and Boot at admission. Later owner presence has not been reconciled. There is no local Body Plan or Play; local Birth remains unavailable.".into() },
            ],
            Vec::new(),
            vec![
                PresentationDisclosure { subject: body, level: PresentationDisclosureLevel::Primary },
                PresentationDisclosure { subject: part, level: PresentationDisclosureLevel::Primary },
                PresentationDisclosure { subject: host, level: PresentationDisclosureLevel::Context },
            ],
        ).map_err(|_| Error::Presentation)
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
    use sha2::{Digest, Sha256};

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
            route_certificates: Vec::new(),
            owner_exchange: OwnerExchange::NotAttempted,
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
        let certificate = vec![42; 128];
        let binding: [u8; 32] = Sha256::digest(&certificate).into();
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
                    "transport_binding_sha256": binding
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
            route_certificates: vec![crate::spore_provision::RouteCertificate {
                candidate_id: "candidate/owner".into(),
                certificate_der: certificate,
            }],
            owner_exchange: OwnerExchange::NotAttempted,
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

        door.joining.as_mut().unwrap().0.owner_exchange =
            OwnerExchange::Refused("virtio-net-absent");
        let refused = door.presentation().unwrap();
        assert!(refused.text.iter().any(|text| {
            text.text.contains("virtio-net-absent") && text.text.contains("unjoined")
        }));
        assert!(refused.basis.body_id.is_none());

        door.joining.as_mut().unwrap().0.owner_exchange = OwnerExchange::ReceiptVerified;
        let verified = door.presentation().unwrap();
        assert!(verified.properties.iter().any(|property| {
            property.name == "owner-receipt-verified"
                && property.value == PresentationPropertyValue::Flag(true)
        }));
        assert!(
            verified
                .text
                .iter()
                .any(|text| { text.text.contains("has not installed membership yet") })
        );
        assert!(verified.basis.body_id.is_none());
        assert!(verified.actions.is_empty());
    }
}
