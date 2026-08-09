#![no_std]

extern crate alloc;

mod lifecycle;
mod lifecycle_events;
mod lifecycle_identity;
mod lifecycle_validation;

pub use lifecycle::*;
pub use lifecycle_events::{ActivationLifecycleEvent, DeploymentLifecycleEvent};
pub use lifecycle_identity::{ActivationId, DeploymentId, MAX_LIFECYCLE_ID_BYTES};

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
mod tests;

#[cfg(test)]
mod lifecycle_tests;
