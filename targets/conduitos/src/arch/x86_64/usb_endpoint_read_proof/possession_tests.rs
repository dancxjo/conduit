//! Explicit appliance issuance fixtures; no initialized-device or DMA claim.
use super::*;
use crate::usb_base::{
    endpoint_read_offer::capture_offer,
    hid_source_plan::{self, HidSourceRole},
};

#[test]
fn eight_selected_endpoint_owners_have_distinct_table_scoped_possession() {
    let contract = EndpointReadContract::prepare().unwrap();
    let subject = EndpointReadProofSubject {
        host_id: "fixture/native-capture-host",
        boot_id: "fixture/native-capture-boot",
        controller_base_id: "fixture/native-capture-base",
        device_instance_id: "fixture/native-capture-device",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
        endpoint_dci: 3,
        endpoint_epoch: 1,
    };
    let (mut host, mut grants) = planning::fixture_host_and_grants(&contract, &subject).unwrap();
    host.capabilities[0] = capture_offer(
        &contract,
        "fixture/native-capture@1".into(),
        "fixture/native-capture-read".into(),
        8,
    )
    .unwrap();
    host.bases[0].capability_ids = vec![host.capabilities[0].capability_id.clone()];
    host.resources[0].capacity_units = 8;
    grants[0].capability_id = host.capabilities[0].capability_id.clone();
    let artifact =
        hid_source_plan::prepare(HidSourceRole::KeyboardCapture, &host, &grants).unwrap();
    let plan = &artifact.artifact().definition().internal_plan;
    let fragment = &plan.fragments[0];
    let placements: Vec<_> = fragment
        .placements
        .iter()
        .filter(|gear| gear.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
        .map(|gear| &gear.placement_id)
        .collect();
    assert_eq!(placements.len(), 8);
    assert!(
        issue(plan).is_err(),
        "single-owner entrance refuses an eight-owner plan"
    );
    let mut owners: Vec<_> = placements
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            // Public deterministic keys belong only to this cooperative fixture.
            issue_selected(plan, placement, [71 + index as u8; 32]).unwrap()
        })
        .collect();
    let expected_claim = owners[0].2.clone();
    for index in 0..8 {
        let (before, remaining) = owners.split_at_mut(index);
        let ((table, handle, claim), after) = remaining.split_first_mut().unwrap();
        assert_eq!(claim, &expected_claim);
        for (_, foreign, _) in before.iter().chain(after.iter()) {
            assert_ne!(handle, foreign);
            assert!(table.authorize(foreign, claim).is_err());
        }
        let mut wrong_play = claim.clone();
        wrong_play.active_play_id = "fixture/other-play".into();
        assert!(table.authorize(handle, &wrong_play).is_err());
        let lease = table.authorize(handle, claim).unwrap();
        table.complete(handle, lease, 0).unwrap();
        table.revoke(handle).unwrap();
        assert!(table.authorize(handle, claim).is_err());
    }
    let pure = fragment
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() != ENDPOINT_READ_IMPLEMENTATION)
        .unwrap();
    assert!(issue_selected(plan, &pure.placement_id, [91; 32]).is_err());
    assert!(issue_selected(plan, &"fixture/missing-placement".into(), [91; 32]).is_err());
}
