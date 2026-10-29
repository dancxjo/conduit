//! Portable wording and inspectable provenance for an Owner-verified source.
use crate::{
    FaceFragment, FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind,
    FaceRole, FaceSubject,
};
use alloc::format;
use conduit_core::HostAdvertisement;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostAttributionRefusal {
    Empty,
    UnknownSubject,
    DuplicateSubject,
    AlreadyAttributed,
    NameTooLong,
    Capacity,
}
impl FaceFragment {
    /// The caller must authenticate the actual measuring Host and this fragment's
    /// execution before using this projection. A serialized advertisement or name
    /// grants no membership, currentness, source authenticity or authority.
    /// Transport/relay Hosts must not be substituted for the measuring Host.
    pub fn with_host_attribution(
        &self,
        host: &HostAdvertisement,
        subjects: &[&str],
    ) -> Result<Self, HostAttributionRefusal> {
        if subjects.len() > crate::face::MAX_FACE_SUBJECTS || self.validate_bounds().is_err() {
            return Err(HostAttributionRefusal::Capacity);
        }
        if subjects.is_empty() {
            return Err(HostAttributionRefusal::Empty);
        }
        let mut digest = Sha256::new();
        for value in [
            "conduit.face/source-host@1",
            self.basis.active_play_id.as_str(),
            host.host_id.as_str(),
            host.boot_id.as_str(),
        ] {
            crate::identity::hash_string(&mut digest, value);
        }
        let source = format!("source-host/{:x}", digest.finalize());
        if self.subjects.iter().any(|s| s.identity == source) {
            return Err(HostAttributionRefusal::AlreadyAttributed);
        }
        if self.relationships.iter().any(|r| {
            subjects.contains(&r.source.as_str())
                && r.kind
                    == FaceRelationshipKind::Semantic(conduit_core::kind_id(
                        "conduit.face/measured-by@1",
                    ))
        }) {
            return Err(HostAttributionRefusal::AlreadyAttributed);
        }
        let mut projected = self.clone();
        for (i, id) in subjects.iter().enumerate() {
            if subjects[i + 1..].contains(id) {
                return Err(HostAttributionRefusal::DuplicateSubject);
            }
            let subject = projected
                .subjects
                .iter_mut()
                .find(|s| s.identity == *id)
                .ok_or(HostAttributionRefusal::UnknownSubject)?;
            let name = format!("{} · {}", host.display_name(), subject.name);
            if name.len() > crate::face::MAX_FACE_TEXT_BYTES {
                return Err(HostAttributionRefusal::NameTooLong);
            }
            subject.name = name;
            projected.relationships.push(FaceRelationship {
                source: (*id).into(),
                target: source.clone(),
                kind: FaceRelationshipKind::Semantic(conduit_core::kind_id(
                    "conduit.face/measured-by@1",
                )),
            });
        }
        projected.subjects.push(FaceSubject {
            identity: source.clone(),
            role: FaceRole::Host,
            name: host.display_name().into(),
        });
        for (name, value) in [
            ("host-id", host.host_id.as_str().into()),
            ("boot-id", host.boot_id.as_str().into()),
            ("offer-generation", format!("{}", host.offer_generation.0)),
        ] {
            projected.properties.push(FaceProperty {
                subject: source.clone(),
                name: name.into(),
                value: FacePropertyValue::Identity(value),
            });
        }
        projected
            .validate_bounds()
            .map_err(|_| HostAttributionRefusal::Capacity)?;
        Ok(projected)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    fn fixture() -> (FaceFragment, HostAdvertisement) {
        let f = FaceFragment {
            basis: crate::FaceContributionBasis {
                checked_plot_id: "checked/test".into(),
                plan_id: "plan/test".into(),
                active_play_id: "play/test".into(),
                required_interaction_context: None,
            },
            subjects: vec![FaceSubject {
                identity: "temperature".into(),
                role: FaceRole::Info,
                name: "Temperature".into(),
            }],
            relationships: vec![],
            composition: vec![],
            properties: vec![],
            text: vec![],
            actions: vec![],
            disclosures: vec![],
            temporal_references: vec![],
            temporal_facts: vec![],
        };
        let h = HostAdvertisement {
            declared_name: Some(conduit_core::HostDeclaredName::new("Yellow").unwrap()),
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: "host/yellow".into(),
            boot_id: "boot/yellow".into(),
            offer_generation: conduit_core::OfferGeneration(2),
            profile: "profile/test".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        };
        (f, h)
    }
    #[test]
    fn declared_source_name_projects_without_changing_execution_or_original() {
        let (f, h) = fixture();
        let p = f.with_host_attribution(&h, &["temperature"]).unwrap();
        assert_eq!(p.subjects[0].name, "Yellow · Temperature");
        assert_eq!(f.subjects[0].name, "Temperature");
        assert_eq!(p.basis, f.basis);
        assert!(p
            .properties
            .iter()
            .any(|x| x.name == "host-id"
                && x.value == FacePropertyValue::Identity("host/yellow".into())));
        assert_eq!(
            p.with_host_attribution(&h, &["temperature"]),
            Err(HostAttributionRefusal::AlreadyAttributed)
        );
    }
    #[test]
    fn duplicate_names_do_not_merge_hosts_and_unnamed_sources_keep_identity() {
        let (f, h) = fixture();
        let a = f.with_host_attribution(&h, &["temperature"]).unwrap();
        let mut other = h.clone();
        other.host_id = "host/other".into();
        let b = f.with_host_attribution(&other, &["temperature"]).unwrap();
        assert_ne!(a.subjects[1].identity, b.subjects[1].identity);
        assert_eq!(
            a.with_host_attribution(&other, &["temperature"]),
            Err(HostAttributionRefusal::AlreadyAttributed)
        );
        other.declared_name = None;
        assert_eq!(
            f.with_host_attribution(&other, &["temperature"])
                .unwrap()
                .subjects[0]
                .name,
            "host/other · Temperature"
        );
        assert_eq!(
            f.with_host_attribution(&h, &["missing"]),
            Err(HostAttributionRefusal::UnknownSubject)
        );
        assert_eq!(
            f.with_host_attribution(&h, &["temperature", "temperature"]),
            Err(HostAttributionRefusal::DuplicateSubject)
        );
    }
}
