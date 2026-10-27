//! Borrowed hashing is encoding only, not verification or effect permission.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/sealed_state.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn borrowed_placement_stream_has_zero_heap_and_identical_refusal_state() {
    let mut f = fixture::fragment();
    f.placements[0].configuration = vec![ConfigurationEntry {
        key: "payload".into(),
        value: ConfigurationValue::Text("é\0".repeat(32768).into()),
    }];
    f.placements[0].resources = vec![ResourceBinding {
        pool_id: "pool".into(),
        class_id: "class".into(),
        units: u32::MAX,
        protected: Some(ProtectedResourceBinding {
            role_id: "role".into(),
            handle_id: "handle".into(),
            access: ProtectedResourceAccess::ReadExisting,
            maximum_bytes: u64::MAX,
            commit_policy: ProtectedResourceCommitPolicy::CreateOnly,
        }),
        compute: Some(ComputeReservation {
            selected_lanes: u32::MAX,
            service_guarantee: ComputeServiceGuarantee::Exclusive,
            architecture_base_id: "architecture".into(),
            architecture_base_kind: ArchitectureBaseKind::BareMetal,
            topology_group_id: Some("group".into()),
            performance_class: Some("performance".into()),
            nominal_clock_hz: Some(u64::MAX),
        }),
        content: Some(ResourceContentOffer {
            contract: ResourceContentRequirement {
                identity: ResourceSemanticIdentity::from_digest([1; 32]),
                version: ResourceVersionIdentity::from_digest([2; 32]),
                content_profile: "content".into(),
                maximum_bytes: u32::MAX,
                maximum_items: u32::MAX,
                retention: ResourceRetention::Play,
                sharing: ResourceSharing::SingleWriterPublished,
                access: ResourceAccessMode::WriteCandidatePublish,
                generation_slots: u16::MAX,
                reader_leases: u16::MAX,
                publication_slots: u16::MAX,
                sensitive: true,
            },
            owner_host: "owner".into(),
            owner_boot: "boot".into(),
            base_id: "residence".into(),
            residence_profile: "profile".into(),
        }),
    }];
    f.placements[0].authority = vec![AuthorityBinding {
        grant_id: "grant".into(),
        contract_id: "contract".into(),
        host_call_contract_id: "call".into(),
        subject_kind: "subject".into(),
        host_id: "host".into(),
        boot_id: "boot".into(),
        capability_id: "capability".into(),
    }];
    let gear = &f.placements[0];
    let resources = [ResourceBindingRef::from(&gear.resources[0])];
    let authorities = [AuthorityBindingRef::from(&gear.authority[0])];
    let (mut builder, observation) = allocation_probe::observe(|| {
        let mut builder = FragmentFingerprintBuilder::new((&f).into(), 1);
        let mut view = PlannedGearRef::from(gear);
        view.resources = ResourceBindingRefs::Borrowed(&resources);
        view.authority = AuthorityBindingRefs::Borrowed(&authorities);
        builder.push_placement_ref(view).unwrap();
        builder
    });
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
    assert_eq!(
        builder.finish((&f).into()).unwrap(),
        compute_fragment_id(&f)
    );
    for expected in [0, 1, 2] {
        let mut rich = FragmentFingerprintBuilder::new((&f).into(), expected);
        let mut borrowed = FragmentFingerprintBuilder::new((&f).into(), expected);
        for _ in 0..3 {
            assert_eq!(
                rich.push_placement(gear),
                borrowed.push_placement_ref(gear.into())
            );
        }
        assert_eq!(rich.finish((&f).into()), borrowed.finish((&f).into()));
    }
    let missing = FragmentFingerprintBuilder::new((&f).into(), 1);
    assert_eq!(
        missing.finish((&f).into()),
        Err(FragmentFingerprintRefusal::IncompletePlacements)
    );
    if let Some(overflow) = (u32::MAX as usize).checked_add(1) {
        builder = FragmentFingerprintBuilder::new((&f).into(), overflow);
        assert_eq!(
            builder.push_placement_ref(gear.into()),
            Err(FragmentFingerprintRefusal::PlacementCountOverflow)
        );
        assert_eq!(
            builder.finish((&f).into()),
            Err(FragmentFingerprintRefusal::PlacementCountOverflow)
        );
    }
}
