//! Host allocation evidence for the autonomous preparation boundary. Root,
//! authority and radio fixtures remain those of the owning CPU execution test.
use super::{allocation_probe, c};

pub fn measure(
    host: &c::HostAdvertisement,
    bases: &[c::BaseImplementationId],
    grants: &[c::AuthorityGrant],
    expected: &c::Plan,
) {
    let (source, compilation) =
        allocation_probe::observe(|| conduit_environment_make::compile_network_setup().unwrap());
    let image = conduit_plot::flat_image::encode(&source).unwrap();
    for absent in [
        host.host_id.as_str(),
        host.boot_id.as_str(),
        "fixture-password",
    ] {
        assert!(!image.windows(absent.len()).any(|v| v == absent.as_bytes()));
    }
    // SAFETY: bytes were produced above from the ordinary checker in this same
    // trusted process. No network input or supplied provenance is accepted.
    let (restored, decoding) = allocation_probe::observe(|| unsafe {
        conduit_plot::flat_image::decode_trusted(&image).unwrap()
    });
    assert_eq!(restored, source);
    // Match Root's fresh preparation overlap: its current advertisement,
    // grants and decoded linked Source coexist with ordinary planning/lowering.
    // The linked image itself is immutable flash on the MCU. CPU fixture
    // advertisement sizes are not a native allocator admission bound.
    let (fresh, combined) = allocation_probe::observe(|| {
        let host = host.clone();
        let grants = grants.to_vec();
        // SAFETY: the image was checked and encoded above in this process.
        let source = unsafe { conduit_plot::flat_image::decode_trusted(&image).unwrap() };
        conduit_environment_make::PreparedNetworkSetup::from_checked(&source, &host, bases, &grants)
            .unwrap()
    });
    assert_eq!(fresh.plan(), expected);
    let (kernel, installation) = allocation_probe::observe(|| {
        conduit_network_join_call::PreparedNetworkKernel::prepare(
            &fresh.plan().fragments[0],
            fresh.lowered(),
        )
        .unwrap()
    });
    eprintln!(
        "network_fresh_preparation pointer_bits={} preparation={combined:?} kernel_installation={installation:?}",
        usize::BITS,
    );
    drop(kernel);
    drop(fresh);
    checked_preparation(&restored, host, bases, grants, expected);
    let mut corrupt: serde_json::Value = serde_json::from_slice(&image).unwrap();
    corrupt["provenance"][0]["construction_owner"] = "not-compiler-authority".into();
    // Negative representation probe only; no returned graph is used for planning.
    assert!(matches!(
        unsafe { conduit_plot::flat_image::decode_trusted(&serde_json::to_vec(&corrupt).unwrap()) },
        Err(conduit_plot::flat_image::ImageRefusal::Representation)
    ));
    let source = restored;
    let (copy, graph) = allocation_probe::observe(|| source.clone());
    drop(copy);
    let (plan, planning) = allocation_probe::observe(|| {
        // Include the retained graph itself in the measured preparation peak;
        // measuring only planner scratch would understate the MCU requirement.
        let source = source.clone();
        let hosts = core::slice::from_ref(host);
        let placements = conduit_planner::default_expanded_placements(&source, hosts).unwrap();
        conduit_planner::plan_expanded_canonical_with_options(
            &source,
            hosts,
            &placements,
            bases,
            conduit_planner::PlanningOptions {
                connection_bases: &std::collections::BTreeMap::new(),
                line_candidates: &std::collections::BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
                authority_grants: grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap()
    });
    assert_eq!(&plan, expected);
    assert!(c::verify_plan(&plan));
    let (lowered, lowering) = allocation_probe::observe(|| {
        // Lowering also needs the live Plan, not just its own scratch storage.
        let plan = plan.clone();
        conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap()
    });
    assert_eq!((lowered.nodes.len(), lowered.cords.len()), (3, 2));
    eprintln!(
        "network_preparation_stages source_peak={} graph_clone_peak={} graph_and_planner_peak={} plan_and_lowering_peak={} planner_allocations={} lowering_allocations={} image_bytes={} image_decode_peak={}",
        compilation.peak_bytes, graph.peak_bytes, planning.peak_bytes,
        lowering.peak_bytes, planning.allocations, lowering.allocations, image.len(), decoding.peak_bytes,
    );
}

fn checked_preparation(
    source: &conduit_plot::ExpandedCanonicalPlot,
    host: &c::HostAdvertisement,
    bases: &[c::BaseImplementationId],
    grants: &[c::AuthorityGrant],
    expected: &c::Plan,
) {
    use conduit_environment_make::PreparedNetworkSetup as P;
    let prepared = P::from_checked(source, host, bases, grants).unwrap();
    assert_eq!(prepared.plan(), expected);
    use conduit_network_join_call::{ControlKernelRefusal, PreparedNetworkKernel};
    let fragment = &prepared.plan().fragments[0];
    let mut changed = prepared.lowered().clone();
    changed.node_specs[0].maximum_step_fuel += 1;
    assert!(matches!(
        PreparedNetworkKernel::prepare(fragment, &changed),
        Err(ControlKernelRefusal::Lowering)
    ));
    let mut foreign = fragment.clone();
    foreign.plan_id = "plan/not-canonical".into();
    assert!(matches!(
        PreparedNetworkKernel::prepare(&foreign, prepared.lowered()),
        Err(ControlKernelRefusal::Identity)
    ));

    assert_eq!(
        P::from_checked(source, host, bases, &[])
            .err()
            .unwrap()
            .code(),
        "authority-missing"
    );
    let mut unavailable = host.clone();
    unavailable.capabilities.clear();
    assert_eq!(
        P::from_checked(source, &unavailable, bases, grants)
            .err()
            .unwrap()
            .code(),
        "capability-unavailable"
    );
    // CPU authority fixture only: changing the Boot requires fresh matching
    // grants and produces a fresh ordinary Plan, never a patched old identity.
    let mut next = host.clone();
    next.boot_id = "boot/next-planning-fixture".into();
    assert_eq!(
        P::from_checked(source, &next, bases, grants)
            .err()
            .unwrap()
            .code(),
        "authority-missing"
    );
    let mut next_grants = grants.to_vec();
    for grant in &mut next_grants {
        grant.boot_id = next.boot_id.clone();
    }
    let next = P::from_checked(source, &next, bases, &next_grants).unwrap();
    assert_ne!(
        next.plan().fragments[0].plan_id,
        expected.fragments[0].plan_id
    );
    assert!(c::verify_plan(next.plan()));
}
