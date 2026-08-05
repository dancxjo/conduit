#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{BootId, HostAdvertisement, HostId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RealmId(String);

impl RealmId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RealmId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LinkId(String);

impl LinkId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for LinkId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkState {
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmLink {
    pub link_id: LinkId,
    pub remote_host_id: HostId,
    pub state: LinkState,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembershipState {
    Active,
    Denied,
    Departed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmMember {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub state: MembershipState,
    pub links: Vec<RealmLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmView {
    pub realm_id: RealmId,
    pub observer_host_id: HostId,
    pub observer_boot_id: BootId,
    pub members: Vec<RealmMember>,
    pub evidence_gap_count: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdmissionRejection {
    StaleBoot,
    DuplicateHost,
    DeniedByPolicy,
    LinkAlreadyBound,
    UnknownHost,
    UnknownLink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealmEvent {
    RealmFounded {
        realm_id: RealmId,
        founder_host_id: HostId,
        founder_boot_id: BootId,
    },
    AdmissionAccepted {
        realm_id: RealmId,
        host_id: HostId,
        boot_id: BootId,
        link_id: LinkId,
    },
    AdmissionRejected {
        realm_id: RealmId,
        host_id: HostId,
        boot_id: BootId,
        link_id: LinkId,
        reason: AdmissionRejection,
    },
    LinkAdded {
        realm_id: RealmId,
        host_id: HostId,
        link_id: LinkId,
    },
    LinkStateChanged {
        realm_id: RealmId,
        host_id: HostId,
        link_id: LinkId,
        state: LinkState,
    },
    MemberDeparted {
        realm_id: RealmId,
        host_id: HostId,
        boot_id: BootId,
    },
    MemberRestored {
        realm_id: RealmId,
        host_id: HostId,
        previous_boot_id: BootId,
        restored_boot_id: BootId,
        link_id: LinkId,
    },
    EvidenceGap {
        dropped: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRequest {
    pub advertisement: HostAdvertisement,
    pub link_id: LinkId,
    pub allow: bool,
}

pub struct Realm {
    realm_id: RealmId,
    members: BTreeMap<HostId, RealmMember>,
    evidence: Vec<RealmEvent>,
    evidence_capacity: usize,
    dropped_evidence: u64,
}

impl Realm {
    pub fn found(
        realm_id: RealmId,
        founder: HostAdvertisement,
        link_id: LinkId,
        evidence_capacity: usize,
    ) -> Self {
        let mut realm = Self {
            realm_id: realm_id.clone(),
            members: BTreeMap::new(),
            evidence: Vec::new(),
            evidence_capacity,
            dropped_evidence: 0,
        };
        realm.members.insert(
            founder.host_id.clone(),
            RealmMember {
                host_id: founder.host_id.clone(),
                boot_id: founder.boot_id.clone(),
                state: MembershipState::Active,
                links: vec![RealmLink {
                    link_id,
                    remote_host_id: founder.host_id.clone(),
                    state: LinkState::Up,
                }],
            },
        );
        realm.record(RealmEvent::RealmFounded {
            realm_id,
            founder_host_id: founder.host_id,
            founder_boot_id: founder.boot_id,
        });
        realm
    }

    pub fn realm_id(&self) -> &RealmId {
        &self.realm_id
    }

    pub fn evidence(&self) -> &[RealmEvent] {
        &self.evidence
    }

    pub fn dropped_evidence(&self) -> u64 {
        self.dropped_evidence
    }

    pub fn admit(&mut self, request: AdmissionRequest) -> Result<(), AdmissionRejection> {
        if !request.allow {
            self.record(RealmEvent::AdmissionRejected {
                realm_id: self.realm_id.clone(),
                host_id: request.advertisement.host_id,
                boot_id: request.advertisement.boot_id,
                link_id: request.link_id,
                reason: AdmissionRejection::DeniedByPolicy,
            });
            return Err(AdmissionRejection::DeniedByPolicy);
        }

        if self
            .members
            .values()
            .flat_map(|member| &member.links)
            .any(|link| link.link_id == request.link_id)
        {
            self.record(RealmEvent::AdmissionRejected {
                realm_id: self.realm_id.clone(),
                host_id: request.advertisement.host_id,
                boot_id: request.advertisement.boot_id,
                link_id: request.link_id,
                reason: AdmissionRejection::LinkAlreadyBound,
            });
            return Err(AdmissionRejection::LinkAlreadyBound);
        }

        if let Some(existing) = self.members.get(&request.advertisement.host_id) {
            let reason = if existing.boot_id == request.advertisement.boot_id {
                AdmissionRejection::DuplicateHost
            } else {
                AdmissionRejection::StaleBoot
            };
            self.record(RealmEvent::AdmissionRejected {
                realm_id: self.realm_id.clone(),
                host_id: request.advertisement.host_id,
                boot_id: request.advertisement.boot_id,
                link_id: request.link_id,
                reason,
            });
            return Err(reason);
        }

        self.members.insert(
            request.advertisement.host_id.clone(),
            RealmMember {
                host_id: request.advertisement.host_id.clone(),
                boot_id: request.advertisement.boot_id.clone(),
                state: MembershipState::Active,
                links: vec![RealmLink {
                    link_id: request.link_id.clone(),
                    remote_host_id: request.advertisement.host_id.clone(),
                    state: LinkState::Up,
                }],
            },
        );
        self.record(RealmEvent::AdmissionAccepted {
            realm_id: self.realm_id.clone(),
            host_id: request.advertisement.host_id,
            boot_id: request.advertisement.boot_id,
            link_id: request.link_id,
        });
        Ok(())
    }

    pub fn add_link(
        &mut self,
        host_id: &HostId,
        link_id: LinkId,
    ) -> Result<(), AdmissionRejection> {
        if self
            .members
            .values()
            .flat_map(|member| &member.links)
            .any(|link| link.link_id == link_id)
        {
            return Err(AdmissionRejection::LinkAlreadyBound);
        }
        let member = self
            .members
            .get_mut(host_id)
            .ok_or(AdmissionRejection::UnknownHost)?;
        member.links.push(RealmLink {
            link_id: link_id.clone(),
            remote_host_id: host_id.clone(),
            state: LinkState::Up,
        });
        self.record(RealmEvent::LinkAdded {
            realm_id: self.realm_id.clone(),
            host_id: host_id.clone(),
            link_id,
        });
        Ok(())
    }

    pub fn set_link_state(
        &mut self,
        host_id: &HostId,
        link_id: &LinkId,
        state: LinkState,
    ) -> Result<(), AdmissionRejection> {
        let member = self
            .members
            .get_mut(host_id)
            .ok_or(AdmissionRejection::UnknownHost)?;
        let link = member
            .links
            .iter_mut()
            .find(|link| &link.link_id == link_id)
            .ok_or(AdmissionRejection::UnknownLink)?;
        link.state = state;
        self.record(RealmEvent::LinkStateChanged {
            realm_id: self.realm_id.clone(),
            host_id: host_id.clone(),
            link_id: link_id.clone(),
            state,
        });
        Ok(())
    }

    pub fn depart(&mut self, host_id: &HostId) -> Result<(), AdmissionRejection> {
        let member = self
            .members
            .get_mut(host_id)
            .ok_or(AdmissionRejection::UnknownHost)?;
        member.state = MembershipState::Departed;
        let boot_id = member.boot_id.clone();
        self.record(RealmEvent::MemberDeparted {
            realm_id: self.realm_id.clone(),
            host_id: host_id.clone(),
            boot_id,
        });
        Ok(())
    }

    pub fn restore_member(
        &mut self,
        advertisement: HostAdvertisement,
        link_id: LinkId,
    ) -> Result<(), AdmissionRejection> {
        if self
            .members
            .values()
            .flat_map(|member| &member.links)
            .any(|link| link.link_id == link_id)
        {
            return Err(AdmissionRejection::LinkAlreadyBound);
        }

        let member = self
            .members
            .get_mut(&advertisement.host_id)
            .ok_or(AdmissionRejection::UnknownHost)?;
        if member.boot_id == advertisement.boot_id {
            return Err(AdmissionRejection::DuplicateHost);
        }

        let previous_boot_id = member.boot_id.clone();
        member.boot_id = advertisement.boot_id.clone();
        member.state = MembershipState::Active;
        member.links.push(RealmLink {
            link_id: link_id.clone(),
            remote_host_id: advertisement.host_id.clone(),
            state: LinkState::Up,
        });
        self.record(RealmEvent::MemberRestored {
            realm_id: self.realm_id.clone(),
            host_id: advertisement.host_id,
            previous_boot_id,
            restored_boot_id: advertisement.boot_id,
            link_id,
        });
        Ok(())
    }

    pub fn view_for(&self, observer: &HostId) -> Option<RealmView> {
        let observer_member = self.members.get(observer)?;
        Some(RealmView {
            realm_id: self.realm_id.clone(),
            observer_host_id: observer_member.host_id.clone(),
            observer_boot_id: observer_member.boot_id.clone(),
            members: self.members.values().cloned().collect(),
            evidence_gap_count: self.dropped_evidence,
        })
    }

    fn record(&mut self, event: RealmEvent) {
        if self.evidence_capacity == 0 {
            self.dropped_evidence += 1;
            return;
        }

        if self.dropped_evidence == 0 {
            self.evidence.push(event);
            if self.evidence.len() <= self.evidence_capacity {
                return;
            }
            self.evidence.remove(0);
            self.dropped_evidence += 1;
            self.ensure_evidence_gap();
            return;
        }

        self.ensure_evidence_gap();
        self.evidence.push(event);
        while self.evidence.len() > self.evidence_capacity {
            let remove_index =
                if matches!(self.evidence.first(), Some(RealmEvent::EvidenceGap { .. }))
                    && self.evidence.len() > 1
                {
                    1
                } else {
                    0
                };
            self.evidence.remove(remove_index);
            self.dropped_evidence += 1;
        }
        self.update_evidence_gap();
    }

    fn ensure_evidence_gap(&mut self) {
        if matches!(self.evidence.first(), Some(RealmEvent::EvidenceGap { .. })) {
            self.update_evidence_gap();
            return;
        }

        if self.evidence_capacity == 1 {
            self.dropped_evidence += self.evidence.len() as u64;
            self.evidence.clear();
            self.evidence.push(RealmEvent::EvidenceGap {
                dropped: self.dropped_evidence,
            });
            return;
        }

        if self.evidence.len() == self.evidence_capacity {
            self.evidence.remove(0);
            self.dropped_evidence += 1;
        }
        self.evidence.insert(
            0,
            RealmEvent::EvidenceGap {
                dropped: self.dropped_evidence,
            },
        );
    }

    fn update_evidence_gap(&mut self) {
        if let Some(RealmEvent::EvidenceGap { dropped }) = self.evidence.first_mut() {
            *dropped = self.dropped_evidence;
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{
        AdmissionRejection, AdmissionRequest, LinkId, LinkState, MembershipState, Realm,
        RealmEvent, RealmId,
    };
    use conduit_core::{
        ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
        HostAdvertisement, HostId, HostProfileId, ImplementationId, KindContractRevision, KindId,
        OfferGeneration, PortDescriptor, PortDirection, PortId, PROTOCOL_VERSION,
    };

    fn advertisement(host: &str, boot: &str) -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from(host),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("test-host"),
            capabilities: vec![CapabilityOffer {
                capability_id: CapabilityId::from("capability-1"),
                kind_id: KindId::from("test/kind"),
                kind_contract_revision: KindContractRevision::from("test/kind@1"),
                execution_profile_id: ExecutionProfileId::from("test/profile@1"),
                implementation_id: ImplementationId::from("test/implementation"),
                artifact_id: ArtifactId::from("test/artifact"),
                inputs: vec![],
                outputs: vec![PortDescriptor {
                    port_id: PortId::from("out"),
                    value_kind: KindId::from("test/value"),
                    direction: PortDirection::Output,
                }],
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            }],
        }
    }

    #[test]
    fn founded_realm_keeps_realm_host_boot_and_link_identities_distinct() {
        let realm = Realm::found(
            RealmId::from("realm-alpha"),
            advertisement("host-a", "boot-a"),
            LinkId::from("link-a"),
            8,
        );
        let view = realm
            .view_for(&HostId::from("host-a"))
            .expect("founder can observe realm");
        assert_eq!(view.realm_id.as_str(), "realm-alpha");
        assert_eq!(view.observer_host_id.as_str(), "host-a");
        assert_eq!(view.observer_boot_id.as_str(), "boot-a");
        assert_eq!(view.members[0].links[0].link_id.as_str(), "link-a");
        assert_ne!(view.realm_id.as_str(), view.observer_host_id.as_str());
        assert_ne!(
            view.observer_host_id.as_str(),
            view.observer_boot_id.as_str()
        );
        assert_ne!(
            view.observer_host_id.as_str(),
            view.members[0].links[0].link_id.as_str()
        );
    }

    #[test]
    fn admits_three_hosts_to_same_realm_view() {
        let mut realm = Realm::found(
            RealmId::from("realm-alpha"),
            advertisement("host-a", "boot-a"),
            LinkId::from("link-a"),
            16,
        );
        realm
            .admit(AdmissionRequest {
                advertisement: advertisement("host-b", "boot-b"),
                link_id: LinkId::from("link-b"),
                allow: true,
            })
            .expect("host-b joins");
        realm
            .admit(AdmissionRequest {
                advertisement: advertisement("host-c", "boot-c"),
                link_id: LinkId::from("link-c"),
                allow: true,
            })
            .expect("host-c joins");
        for host in ["host-a", "host-b", "host-c"] {
            let view = realm
                .view_for(&HostId::from(host))
                .expect("active host sees realm");
            assert_eq!(view.members.len(), 3);
            assert!(view
                .members
                .iter()
                .all(|member| member.state == MembershipState::Active));
        }
    }

    #[test]
    fn duplicate_stale_denied_and_departed_are_distinct() {
        let mut realm = Realm::found(
            RealmId::from("realm-alpha"),
            advertisement("host-a", "boot-a"),
            LinkId::from("link-a"),
            16,
        );
        assert_eq!(
            realm.admit(AdmissionRequest {
                advertisement: advertisement("host-a", "boot-a"),
                link_id: LinkId::from("link-duplicate"),
                allow: true,
            }),
            Err(AdmissionRejection::DuplicateHost)
        );
        assert_eq!(
            realm.admit(AdmissionRequest {
                advertisement: advertisement("host-a", "boot-stale"),
                link_id: LinkId::from("link-stale"),
                allow: true,
            }),
            Err(AdmissionRejection::StaleBoot)
        );
        assert_eq!(
            realm.admit(AdmissionRequest {
                advertisement: advertisement("host-denied", "boot-denied"),
                link_id: LinkId::from("link-denied"),
                allow: false,
            }),
            Err(AdmissionRejection::DeniedByPolicy)
        );
        realm.depart(&HostId::from("host-a")).expect("host departs");
        let view = realm
            .view_for(&HostId::from("host-a"))
            .expect("departed host remains inspectable");
        assert_eq!(view.members[0].state, MembershipState::Departed);
        assert!(realm.evidence().iter().any(|event| matches!(
            event,
            RealmEvent::AdmissionRejected {
                reason: AdmissionRejection::DuplicateHost,
                ..
            }
        )));
        assert!(realm.evidence().iter().any(|event| matches!(
            event,
            RealmEvent::AdmissionRejected {
                reason: AdmissionRejection::StaleBoot,
                ..
            }
        )));
        assert!(realm.evidence().iter().any(|event| matches!(
            event,
            RealmEvent::AdmissionRejected {
                reason: AdmissionRejection::DeniedByPolicy,
                ..
            }
        )));
        assert!(realm
            .evidence()
            .iter()
            .any(|event| matches!(event, RealmEvent::MemberDeparted { .. })));
    }

    #[test]
    fn multiple_links_do_not_duplicate_membership_and_lost_link_preserves_host() {
        let mut realm = Realm::found(
            RealmId::from("realm-alpha"),
            advertisement("host-a", "boot-a"),
            LinkId::from("link-a"),
            16,
        );
        realm
            .admit(AdmissionRequest {
                advertisement: advertisement("host-b", "boot-b"),
                link_id: LinkId::from("link-b1"),
                allow: true,
            })
            .expect("host-b joins");
        realm
            .add_link(&HostId::from("host-b"), LinkId::from("link-b2"))
            .expect("second link is path, not membership");
        let view = realm
            .view_for(&HostId::from("host-a"))
            .expect("host-a sees realm");
        let host_b = view
            .members
            .iter()
            .find(|member| member.host_id == HostId::from("host-b"))
            .expect("host-b remains one member");
        assert_eq!(view.members.len(), 2);
        assert_eq!(host_b.links.len(), 2);

        realm
            .set_link_state(
                &HostId::from("host-b"),
                &LinkId::from("link-b1"),
                LinkState::Down,
            )
            .expect("link loss is represented");
        let view = realm
            .view_for(&HostId::from("host-a"))
            .expect("host-a sees realm");
        let host_b = view
            .members
            .iter()
            .find(|member| member.host_id == HostId::from("host-b"))
            .expect("host-b remains after link loss");
        assert_eq!(host_b.state, MembershipState::Active);
        assert_eq!(
            host_b
                .links
                .iter()
                .find(|link| link.link_id == LinkId::from("link-b1"))
                .expect("first link remains")
                .state,
            LinkState::Down
        );
    }

    #[test]
    fn restart_requires_explicit_restore_or_new_realm() {
        let mut realm = Realm::found(
            RealmId::from("realm-alpha"),
            advertisement("host-a", "boot-a"),
            LinkId::from("link-a"),
            16,
        );
        assert_eq!(
            realm.admit(AdmissionRequest {
                advertisement: advertisement("host-a", "boot-restarted"),
                link_id: LinkId::from("link-implicit-restart"),
                allow: true,
            }),
            Err(AdmissionRejection::StaleBoot)
        );

        realm
            .restore_member(
                advertisement("host-a", "boot-restarted"),
                LinkId::from("link-restored"),
            )
            .expect("operator explicitly restores host into existing realm");
        let restored_view = realm
            .view_for(&HostId::from("host-a"))
            .expect("restored host sees existing realm");
        assert_eq!(restored_view.realm_id, RealmId::from("realm-alpha"));
        assert_eq!(
            restored_view.observer_boot_id,
            BootId::from("boot-restarted")
        );
        assert_eq!(restored_view.members.len(), 1);
        assert_eq!(restored_view.members[0].links.len(), 2);
        assert!(realm.evidence().iter().any(|event| matches!(
            event,
            RealmEvent::MemberRestored {
                previous_boot_id,
                restored_boot_id,
                ..
            } if previous_boot_id == &BootId::from("boot-a")
                && restored_boot_id == &BootId::from("boot-restarted")
        )));

        let new_realm = Realm::found(
            RealmId::from("realm-beta"),
            advertisement("host-a", "boot-restarted"),
            LinkId::from("link-new-realm"),
            16,
        );
        let new_view = new_realm
            .view_for(&HostId::from("host-a"))
            .expect("host can intentionally found a new realm after restart");
        assert_eq!(new_view.realm_id, RealmId::from("realm-beta"));
        assert_ne!(new_view.realm_id, restored_view.realm_id);
    }

    #[test]
    fn evidence_is_bounded_and_reports_gaps() {
        let mut realm = Realm::found(
            RealmId::from("realm-alpha"),
            advertisement("host-a", "boot-a"),
            LinkId::from("link-a"),
            3,
        );
        for index in 0..6 {
            let _ = realm.admit(AdmissionRequest {
                advertisement: advertisement("host-a", "boot-a"),
                link_id: LinkId::from(alloc::format!("duplicate-{index}").as_str()),
                allow: true,
            });
        }
        assert_eq!(realm.evidence().len(), 3);
        assert_eq!(realm.dropped_evidence(), 5);
        assert!(matches!(
            realm.evidence().first(),
            Some(RealmEvent::EvidenceGap { dropped }) if *dropped == realm.dropped_evidence()
        ));
        let view = realm
            .view_for(&HostId::from("host-a"))
            .expect("view remains available");
        assert_eq!(view.evidence_gap_count, realm.dropped_evidence());
    }
}
