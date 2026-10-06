//! Fixture advertisements exercise the production entrance; fixture authority
//! remains confined to this test module.
use super::*;
use crate::usb_base::{
    endpoint_read_contract::EndpointReadContract,
    endpoint_read_proof_plan::{EndpointReadProofSubject, host_and_grants},
};

fn fixture() -> (HostAdvertisement, [AuthorityGrant; 1]) {
    let contract = EndpointReadContract::prepare().unwrap();
    let subject = EndpointReadProofSubject {
        host_id: "hid-admission-test-host",
        boot_id: "hid-admission-test-boot",
        controller_base_id: "hid-admission-test-controller",
        device_instance_id: "hid-admission-test-device",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
        endpoint_dci: 3,
        endpoint_epoch: 1,
    };
    let (host, mut grants) = host_and_grants(&contract, &subject).unwrap();
    grants[0].grant_id = "operator/hid-read".into();
    (host, grants)
}

#[test]
fn both_roles_require_explicit_current_authority() {
    let (host, grants) = fixture();
    let before = serde_json::to_vec(&host).unwrap();
    for role in [HidSourceRole::Keyboard, HidSourceRole::Mouse] {
        let artifact = prepare(role, &host, &grants).unwrap();
        let plan = &artifact.artifact().definition().internal_plan;
        let bindings: alloc::vec::Vec<_> = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .flat_map(|gear| &gear.authority)
            .collect();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].grant_id, grants[0].grant_id);
        assert_eq!(bindings[0].host_id, host.host_id);
        assert_eq!(bindings[0].boot_id, host.boot_id);
        assert!(prepare(role, &host, &[]).is_err());
        let mut stale = grants.clone();
        stale[0].boot_id = "previous-boot".into();
        assert!(prepare(role, &host, &stale).is_err());
        let mut wrong_host = grants.clone();
        wrong_host[0].host_id = "another-host".into();
        assert!(prepare(role, &host, &wrong_host).is_err());
    }
    assert_eq!(serde_json::to_vec(&host).unwrap(), before);
}

#[test]
fn authority_does_not_create_an_endpoint_offer() {
    let (mut host, grants) = fixture();
    host.capabilities.clear();
    host.bases.clear();
    host.resources.clear();
    for role in [HidSourceRole::Keyboard, HidSourceRole::Mouse] {
        assert!(prepare(role, &host, &grants).is_err());
    }
}
