//! Fresh State and content residence ownership in the same funded local lowerer.
use conduit_core::*;
use conduit_plan_lowering::{
    fragment_set::{
        lower_local_fragment_set_from_plans_with_limit, lower_local_fragment_set_layout_from_plans,
        lowered_local_fragment_set_backing_bytes, FragmentSetBounds, FragmentSetError,
    },
    lowering::{KernelStorageProfile, FIXED_KERNEL_STORAGE_PROFILE},
};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn profile() -> KernelStorageProfile {
    FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(8, 4096)
        .unwrap()
        .with_body_state_residence()
}
fn bounds() -> FragmentSetBounds {
    FragmentSetBounds {
        fragments: 4,
        nodes: 16,
        cords: 16,
        queue_slots: 32,
        value_bytes: 8192,
        sign_items: 128,
        sign_bytes: 8192,
    }
}
fn plan(lifetime: StateLifetime, content: bool) -> Plan {
    let mut fragment = common::fragment();
    let state = &mut fragment.states[0];
    state.lifetime = lifetime;
    state.maximum_value_bytes = 1024;
    state.initial_value = Some(vec![7; 513]);
    fragment.placements[0].limits.max_queue_bytes = 1024;
    if content {
        fragment.placements[0].resources.push(ResourceBinding {
            pool_id: "fixture/json-pool".into(),
            class_id: "resource/snapshot@1".into(),
            units: 1,
            protected: None,
            compute: None,
            content: Some(ResourceContentOffer {
                contract: ResourceContentRequirement {
                    identity: ResourceSemanticIdentity::from_digest([1; 32]),
                    version: ResourceVersionIdentity::from_digest([2; 32]),
                    content_profile: kind_id("text/json-utf8@1"),
                    maximum_bytes: 4096,
                    maximum_items: 1,
                    retention: ResourceRetention::ExternalDurable,
                    sharing: ResourceSharing::SingleWriterPublished,
                    access: ResourceAccessMode::WriteCandidatePublish,
                    generation_slots: 1,
                    reader_leases: 1,
                    publication_slots: 1,
                    sensitive: true,
                },
                owner_host: fragment.host_id.clone(),
                owner_boot: fragment.boot_id.clone(),
                base_id: "fixture/json-directory".into(),
                residence_profile: kind_id("fixture/json@1"),
            }),
        });
    }
    let base = mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    let state = state_resource_budget(&fragment.states).unwrap();
    fragment.sign_storage_budget = SignStorageBudget {
        item_capacity: base.item_capacity + state.sign_storage.item_capacity,
        byte_capacity: base.byte_capacity + state.sign_storage.byte_capacity,
    };
    common::seal(fragment)
}

#[test]
fn fresh_body_state_and_content_fit_actual_owned_lowering_peak_without_descriptor_allocations() {
    for content in [false, true] {
        let plan = plan(StateLifetime::Body, content);
        assert!(verify_plan(&plan));
        let (layout, observed) = allocation_probe::observe(|| {
            lower_local_fragment_set_layout_from_plans(&[&plan], profile(), bounds())
        });
        let layout = layout.unwrap();
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        let (lowered, observed) = allocation_probe::observe(|| {
            lower_local_fragment_set_from_plans_with_limit(
                &[&plan],
                profile(),
                bounds(),
                layout.peak_owned_bytes,
            )
        });
        let lowered = lowered.unwrap();
        assert_eq!(
            lowered.partitions[0].states[0].contract,
            plan.fragments[0].states[0]
        );
        let (retained, inspected) =
            allocation_probe::observe(|| lowered_local_fragment_set_backing_bytes(&lowered));
        assert!(retained.unwrap() <= layout.retained_owned_bytes);
        assert_eq!((inspected.allocations, inspected.reallocations), (0, 0));
        assert!(
            observed.peak_bytes <= layout.peak_owned_bytes,
            "{observed:?}, {layout:?}"
        );
        assert_eq!(observed.reallocations, 0);
        let (refused, observed) = allocation_probe::observe(|| {
            lower_local_fragment_set_from_plans_with_limit(
                &[&plan],
                profile(),
                bounds(),
                layout.peak_owned_bytes - 1,
            )
        });
        assert!(
            matches!(refused, Err(FragmentSetError::Capacity(ref d)) if d.resource == "lowering-owned-bytes")
        );
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    }
}

#[test]
fn body_residence_opt_in_preserves_other_lifetime_and_owned_continuity_refusals() {
    let body = plan(StateLifetime::Body, false);
    let layout = lower_local_fragment_set_layout_from_plans(&[&body], profile(), bounds()).unwrap();
    let ordinary = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(8, 4096)
        .unwrap();
    assert!(matches!(
        lower_local_fragment_set_from_plans_with_limit(
            &[&body],
            ordinary,
            bounds(),
            layout.peak_owned_bytes,
        ),
        Err(FragmentSetError::StateUnsupported)
    ));
    for lifetime in [StateLifetime::Wake, StateLifetime::Boot] {
        let plan = plan(lifetime, false);
        let layout =
            lower_local_fragment_set_layout_from_plans(&[&plan], profile(), bounds()).unwrap();
        assert!(matches!(
            lower_local_fragment_set_from_plans_with_limit(
                &[&plan],
                profile(),
                bounds(),
                layout.peak_owned_bytes,
            ),
            Err(FragmentSetError::StateUnsupported)
        ));
    }
    assert!(!FIXED_KERNEL_STORAGE_PROFILE.supports_body_state_residence());
    assert!(profile().supports_body_state_residence());
    let mut retained = common::retained_fragment();
    retained.states[0].lifetime = StateLifetime::Body;
    let retained = common::seal(retained);
    assert!(local_plan_verification_backing_bytes(&retained).is_none());
    assert!(lower_local_fragment_set_layout_from_plans(&[&retained], profile(), bounds()).is_err());
}
