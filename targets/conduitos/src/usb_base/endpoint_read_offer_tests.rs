use super::*;
use crate::protocol_source::{PreparedProtocolEntry, ProtocolSourcePackage};
use crate::usb_base::endpoint_read_proof_plan::{EndpointReadProofSubject, host_and_grants};
use alloc::{collections::BTreeMap, format, string::String};

#[test]
fn eight_reads_require_eight_host_instances_resources_and_explicit_authority() {
    let mut source = String::from(
        "with machine/usb/endpoint-read/request as Request\nwith machine/usb/endpoint-read/result as Result\nplot capture (\n",
    );
    for index in 0..8 {
        source.push_str(&format!(
            ">> request{index}: Request...| <= 4096B\nresult{index}: Result...| <= 4096B >>\n"
        ));
    }
    source.push_str(") {\n");
    for index in 0..8 {
        source.push_str(&format!("transfer{index}: machine/usb/endpoint-read\nrequest{index} >> transfer{index} >> result{index}\n"));
    }
    source.push_str("}\n");
    let package = ProtocolSourcePackage::compile(source, &[]).unwrap();
    let package_bytes = serde_json::to_vec(&package).unwrap();
    let contract = EndpointReadContract::prepare().unwrap();
    let subject = EndpointReadProofSubject {
        host_id: "capture-admission-fixture-host",
        boot_id: "capture-admission-fixture-boot",
        controller_base_id: "capture-admission-fixture-base",
        device_instance_id: "capture-admission-fixture-provider",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
        endpoint_dci: 3,
        endpoint_epoch: 1,
    };
    let (mut host, mut grants) = host_and_grants(&contract, &subject).unwrap();
    host.capabilities[0] = capture_offer(
        &contract,
        "fixture/capture-admission@1".into(),
        "fixture/capture-read".into(),
        8,
    )
    .unwrap();
    host.bases[0].capability_ids = vec![host.capabilities[0].capability_id.clone()];
    host.resources[0].capacity_units = 8;
    grants[0].capability_id = host.capabilities[0].capability_id.clone();
    let plan = |host: HostAdvertisement, grants: &[AuthorityGrant]| {
        let entry = PreparedProtocolEntry::prepare(&package_bytes, "capture").unwrap();
        let hosts = [host];
        let placements = entry.placements(&hosts)?;
        entry.plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
    };
    let artifact = plan(host.clone(), &grants).unwrap();
    let placements = &artifact.artifact().definition().internal_plan.fragments[0].placements;
    assert_eq!(placements.len(), 8);
    for gear in placements {
        assert_eq!(gear.host_calls.len(), 1);
        assert_eq!(gear.host_calls[0].maximum_in_flight, 1);
        assert_eq!(gear.host_calls[0].maximum_input_bytes, 4096);
        assert_eq!(gear.host_calls[0].maximum_output_bytes, 4096);
        assert_eq!(
            gear.resources
                .iter()
                .map(|binding| binding.units)
                .sum::<u32>(),
            1
        );
    }
    let mut kernel = crate::usb_base::hid_source_kernel::PreparedHidSourceKernel::prepare(
        artifact,
        conduit_composite::KernelCompositeSignStorage::default(),
    )
    .unwrap();
    assert_eq!(
        kernel.kernel_mut().definition().internal_plan.fragments[0]
            .placements
            .len(),
        8
    );
    assert!(plan(host.clone(), &[]).is_err());
    let mut fewer_instances = host.clone();
    fewer_instances.capabilities[0].limits.max_active_instances = 7;
    assert!(plan(fewer_instances, &grants).is_err());
    let mut fewer_resources = host;
    fewer_resources.resources[0].capacity_units = 7;
    assert!(plan(fewer_resources, &grants).is_err());
    for bound in [0, 9] {
        assert!(
            capture_offer(
                &contract,
                "fixture/capture-admission@1".into(),
                "fixture/capture-read".into(),
                bound
            )
            .is_err()
        );
    }
}
