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

#[test]
fn ordered_keyboard_class_requires_eight_admitted_calls_in_the_shared_kernel() {
    let (mut host, mut grants) = fixture();
    let contract = EndpointReadContract::prepare().unwrap();
    host.capabilities[0] = crate::usb_base::endpoint_read_offer::capture_offer(
        &contract,
        "fixture/ordered-keyboard-capture@1".into(),
        "fixture/ordered-keyboard-capture".into(),
        8,
    )
    .unwrap();
    host.bases[0].capability_ids = vec![host.capabilities[0].capability_id.clone()];
    host.resources[0].capacity_units = 8;
    grants[0].capability_id = host.capabilities[0].capability_id.clone();
    let artifact = prepare(HidSourceRole::KeyboardCapture, &host, &grants).unwrap();
    let plan = &artifact.artifact().definition().internal_plan;
    let endpoints: alloc::vec::Vec<_> = plan.fragments[0]
        .placements
        .iter()
        .filter(|gear| {
            gear.implementation_id.as_str()
                == crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION
        })
        .collect();
    assert_eq!(endpoints.len(), 8);
    for endpoint in endpoints {
        assert_eq!(endpoint.host_calls.len(), 1);
        assert_eq!(endpoint.host_calls[0].maximum_in_flight, 1);
        assert_eq!(endpoint.host_calls[0].maximum_input_bytes, 4096);
        assert_eq!(endpoint.host_calls[0].maximum_output_bytes, 4096);
    }
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    assert!(
        lowered.nodes.len() <= 64,
        "nodes={}, cords={}, slots={}, calls={}",
        lowered.nodes.len(),
        lowered.cords.len(),
        lowered.cord_value_slots,
        lowered.host_calls.len()
    );
    let mut run = crate::usb_base::hid_source_kernel::PreparedHidSourceKernel::prepare(
        artifact,
        conduit_composite::KernelCompositeSignStorage::default(),
    )
    .unwrap();
    assert_eq!(run.kernel_mut().definition().boundary.input_fronts.len(), 9);
    assert_eq!(
        run.kernel_mut().definition().boundary.output_fronts.len(),
        3
    );
    assert!(prepare(HidSourceRole::KeyboardCapture, &host, &[]).is_err());
    let mut fewer = host.clone();
    fewer.capabilities[0].limits.max_active_instances = 7;
    assert!(prepare(HidSourceRole::KeyboardCapture, &fewer, &grants).is_err());
    host.resources[0].capacity_units = 7;
    assert!(prepare(HidSourceRole::KeyboardCapture, &host, &grants).is_err());
}

#[test]
fn ordered_mouse_class_requires_two_admitted_calls_in_the_shared_kernel() {
    let (mut host, mut grants) = fixture();
    let contract = EndpointReadContract::prepare().unwrap();
    host.capabilities[0] = crate::usb_base::endpoint_read_offer::capture_offer(
        &contract,
        "fixture/ordered-mouse-capture@1".into(),
        "fixture/ordered-mouse-capture".into(),
        2,
    )
    .unwrap();
    host.bases[0].capability_ids = vec![host.capabilities[0].capability_id.clone()];
    host.resources[0].capacity_units = 2;
    grants[0].capability_id = host.capabilities[0].capability_id.clone();
    let artifact = prepare(HidSourceRole::MouseCapture, &host, &grants).unwrap();
    let plan = &artifact.artifact().definition().internal_plan;
    let endpoints: alloc::vec::Vec<_> = plan.fragments[0]
        .placements
        .iter()
        .filter(|gear| {
            gear.implementation_id.as_str()
                == crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION
        })
        .collect();
    assert_eq!(endpoints.len(), 2);
    for endpoint in endpoints {
        assert_eq!(endpoint.authority.len(), 1);
        assert_eq!(endpoint.authority[0].grant_id, grants[0].grant_id);
        assert_eq!(endpoint.host_calls.len(), 1);
        assert_eq!(endpoint.host_calls[0].maximum_in_flight, 1);
        assert_eq!(endpoint.host_calls[0].maximum_input_bytes, 4096);
        assert_eq!(endpoint.host_calls[0].maximum_output_bytes, 4096);
    }
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    assert!(lowered.nodes.len() <= 64);
    let mut run = crate::usb_base::hid_source_kernel::PreparedHidSourceKernel::prepare(
        artifact,
        conduit_composite::KernelCompositeSignStorage::default(),
    )
    .unwrap();
    assert_eq!(run.kernel_mut().definition().boundary.input_fronts.len(), 3);
    assert_eq!(
        run.kernel_mut().definition().boundary.output_fronts.len(),
        1
    );
    assert!(prepare(HidSourceRole::MouseCapture, &host, &[]).is_err());
    let mut stale = grants.clone();
    stale[0].boot_id = "previous-boot".into();
    assert!(prepare(HidSourceRole::MouseCapture, &host, &stale).is_err());
    let mut fewer = host.clone();
    fewer.capabilities[0].limits.max_active_instances = 1;
    assert!(prepare(HidSourceRole::MouseCapture, &fewer, &grants).is_err());
    host.resources[0].capacity_units = 1;
    assert!(prepare(HidSourceRole::MouseCapture, &host, &grants).is_err());
}
