//! Checked authored scan meaning; this test does not initialize a radio.
#[test]
fn scan_source_checks_with_a_connected_typed_evidence_sink() {
    let plot = conduit_environment_make::compile_network_scan().unwrap();
    assert_eq!(plot.name, "environment-network-scan");
    assert_eq!(plot.gears.len(), 2);
    assert_eq!(plot.connections.len(), 1);
    // Invocation generation and deadlines belong to the retained provider,
    // rather than an invented authored input or remote request port.
    assert!(conduit_net::network_scan_contract().inputs.is_empty());
    let sink = conduit_net::network_scan_sign_contract();
    assert_eq!(sink.inputs.len(), 1);
    assert!(sink.outputs.is_empty());
    // The existing installed join Source must still check with this catalog.
    conduit_environment_make::compile_network_setup().unwrap();
}

// This journey uses ordinary checked Source, initialized CPU-provider offers,
// real planner/lowering and the production scheduler. Host completions below
// are explicitly CPU inputs: physical selection/cleanup is tested separately.
struct Inert;
impl conduit_network_join_call::scan::NetworkScanProvider for Inert {
    fn now_ms(&self) -> u64 {
        1
    }
    async fn scan(
        &mut self,
        _: conduit_net::NetworkScanRequest,
        _: &mut [conduit_net::ScanObservation; 16],
    ) -> Result<conduit_net::ScanCompletion, conduit_net::ScanFailure> {
        panic!("kernel preparation cannot scan")
    }
    async fn cleanup(&mut self, _: u64) -> Result<(), conduit_net::ScanFailure> {
        panic!("kernel preparation cannot clean a radio")
    }
}
fn scan_plan() -> conduit_core::Plan {
    use conduit_core as c;
    use conduit_net::*;
    use conduit_network_join_call::{installation::NetworkNativeIdentity, scan::*};
    use std::collections::BTreeMap;
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "cpu/host".into(),
        boot_id: "cpu/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "cpu/scan".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    // SAFETY: exclusively owned inert CPU provider; methods panic above.
    let ready = unsafe {
        ReadyNetworkScanBase::new(
            NetworkNativeIdentity {
                host: host.host_id.clone(),
                boot: host.boot_id.clone(),
                base: "cpu/base".into(),
                instance: "cpu/instance".into(),
                provider_generation: 1,
                resource: "cpu/radio".into(),
                resource_generation: c::ResourceGenerationId("cpu/radio/1".into()),
                envelope: "cpu/envelope".into(),
                artifact: "cpu/artifact".into(),
                implementation: "cpu/scan@1".into(),
                base_implementation: "cpu/base@1".into(),
            },
            NetworkScanRequest {
                generation: 1,
                scan_deadline_ms: 10,
                cleanup_deadline_ms: 20,
            },
            Inert,
        )
    }
    .unwrap_or_else(|_| panic!("valid CPU initialization"));
    ready.append_to_advertisement(&mut host).unwrap();
    host.capabilities.push(network_scan_sign_offer(
        "cpu/sign".into(),
        "cpu/sign@1".into(),
        "cpu/sign-artifact".into(),
    ));
    let grants = [c::AuthorityGrant {
        grant_id: "cpu/grant".into(),
        contract_id: NETWORK_SCAN_AUTHORITY.into(),
        host_call_contract_id: NETWORK_SCAN_HOST_CALL.into(),
        subject_kind: NETWORK_SCAN_SUBJECT.into(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: SCAN_CAPABILITY.into(),
    }];
    let source = conduit_environment_make::compile_network_scan().unwrap();
    let hosts = [host];
    let placements = conduit_planner::default_expanded_placements(&source, &hosts).unwrap();
    conduit_planner::plan_expanded_canonical_with_options(
        &source,
        &hosts,
        &placements,
        &["conduit.base/local@1".into(), "cpu/base@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_SCAN_OUTPUT_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
}

#[test]
fn actual_checked_scan_kernel_delivers_one_bounded_result_and_retires_original_signs() {
    use conduit_kernel::{self as k, scheduler::SchedulerStatus};
    use conduit_net::*;
    use conduit_network_join_call::scan::*;
    let plan = scan_plan();
    assert_eq!(plan.fragments.len(), 1);
    let parts = PreparedScanKernelParts::prepare(&plan.fragments[0]).unwrap();
    let mut prepared = PreparedScanKernelStorage::try_new()
        .unwrap()
        .finish(parts)
        .unwrap();
    let original = (&*prepared) as *const _;
    prepared = match prepared.try_retire_signs() {
        Ok(_) => panic!("unplayed kernel cannot retire"),
        Err(retained) => retained,
    };
    assert_eq!((&*prepared) as *const _, original);
    let mut observations = [ScanObservation::EMPTY; 16];
    for (i, r) in observations.iter_mut().enumerate() {
        *r = ScanObservation::new(
            format!("cpu-network-{i}").as_bytes(),
            ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal),
            -45,
            1,
        )
        .unwrap();
    }
    let mut wire = [0; MAXIMUM_SCAN_OUTPUT_BYTES as usize];
    let len = ScanCompletion {
        generation: 1,
        completed_ms: 1,
        count: 16,
        total: 17,
    }
    .encode(&observations, &mut wire)
    .unwrap();
    assert_eq!(len, wire.len());
    let mut calls = [0u8; 2];
    let mut drained = false;
    for _ in 0..32 {
        let status = prepared.kernel.step().unwrap();
        while let Some(request) = prepared.kernel.next_host_request() {
            let output = if request.node == prepared.roles.scan {
                calls[0] += 1;
                assert!(prepared
                    .kernel
                    .host_value(request.input.value)
                    .unwrap()
                    .is_empty());
                let value = prepared.kernel.store_host_value(&wire).unwrap();
                Some(k::BoundedValueRef::new(value, MAXIMUM_SCAN_OUTPUT_BYTES).unwrap())
            } else {
                assert_eq!(request.node, prepared.roles.evidence);
                calls[1] += 1;
                assert_eq!(
                    prepared.kernel.host_value(request.input.value).unwrap(),
                    wire
                );
                None
            };
            prepared
                .kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    k::HostCallOutcome {
                        disposition: k::HostCallDisposition::Completed,
                        output,
                        failure: None,
                    },
                )
                .unwrap();
        }
        if status == SchedulerStatus::Drained {
            drained = true;
            break;
        }
    }
    assert!(drained);
    assert_eq!(calls, [1, 1]);
    assert_eq!(prepared.kernel.pending_host_call_count(), 0);
    assert!(prepared.kernel.can_retire());
    assert!(prepared.try_retire_signs().is_ok());
    println!("CPU64 scan storage bytes: {}", SCAN_KERNEL_BYTES);
}

#[test]
fn scan_kernel_refuses_unverified_plan_identity_before_storage() {
    use conduit_network_join_call::scan::*;
    let mut plan = scan_plan();
    plan.fragments[0].boot_id = "foreign/boot".into();
    assert!(matches!(
        PreparedScanKernelParts::prepare(&plan.fragments[0]),
        Err(ScanKernelRefusal::Identity)
    ));
}
