use alloc::vec;

use super::{
    AdmissionRejection, AdmissionRequest, LinkId, LinkState, MembershipState, Realm, RealmEvent,
    RealmId,
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
        resources: vec![],
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
            host_operations: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
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
