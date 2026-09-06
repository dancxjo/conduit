//! Numeric composition proof only; production Body-wide execution is separate.
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_plan_lowering::{
    fragment_set::{lower_local_fragment_set, FragmentSetBounds, FragmentSetError},
    lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PROFILE},
};

fn plans() -> (conduit_core::Plan, conduit_core::Plan) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    let host = conduit_std_host::StdHost::new();
    let make = |source: &str| {
        let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
        let expanded = expand_canonical_form(&checked, "clock-demo", &profile).unwrap();
        host.plan_expanded_local(&expanded).unwrap()
    };
    let source = include_str!("../../../forms/clock/main.conduit");
    (make(source), make(&source.replace("1s", "2s")))
}

fn bounds() -> FragmentSetBounds {
    FragmentSetBounds {
        fragments: 2,
        nodes: 4,
        cords: 2,
        queue_slots: 16,
        value_bytes: 1024,
        sign_items: 1024,
        sign_bytes: 65536,
    }
}

#[test]
fn independent_exact_plans_receive_disjoint_kernel_ids_without_source_identity_rewrite() {
    let (first, second) = plans();
    let snapshot = (first.clone(), second.clone());
    let original = lower_plan_fragment(&second.fragments[0]).unwrap();
    let set = lower_local_fragment_set(
        &[&first.fragments[0], &second.fragments[0]],
        FIXED_KERNEL_STORAGE_PROFILE,
        bounds(),
    )
    .unwrap();
    assert_eq!(set.nodes, 4);
    assert_eq!(set.cords, 2);
    assert_eq!(set.partitions[1].identity.plan_id, second.plan_id);
    assert_eq!(
        set.partitions[1].identity.fragment_id,
        second.fragments[0].fragment_id
    );
    for (old, new) in original.nodes.iter().zip(&set.partitions[1].nodes) {
        assert_eq!(new.node.0, old.node.0 + 2);
        assert_eq!(new.placement_id, old.placement_id);
        assert_eq!(
            set.partitions[1]
                .identity
                .node_for_placement(&old.placement_id),
            Some(new.node)
        );
    }
    let old = &original.cords[0];
    let new = &set.partitions[1].cords[0];
    assert_eq!(new.connection_id, old.connection_id);
    assert_eq!(new.spec.cord.0, old.spec.cord.0 + 1);
    assert_eq!(
        new.spec.slot_start,
        old.spec.slot_start + set.partitions[0].cord_value_slots
    );
    assert_eq!(new.spec.byte_capacity, old.spec.byte_capacity);
    assert_eq!(set.partitions[1].routes[0].targets[0].cord, new.spec.cord);
    assert_eq!(set.partitions[1].routes[0].targets[0].sink, new.spec.sink);
    assert_eq!((first, second), snapshot);
}

#[test]
fn combined_bounds_duplicate_partition_and_changed_boot_refuse() {
    let (first, mut second) = plans();
    let mut small = bounds();
    small.nodes = 3;
    assert_eq!(
        lower_local_fragment_set(
            &[&first.fragments[0], &second.fragments[0]],
            FIXED_KERNEL_STORAGE_PROFILE,
            small
        )
        .unwrap_err(),
        FragmentSetError::Capacity
    );
    assert_eq!(
        lower_local_fragment_set(
            &[&first.fragments[0], &first.fragments[0]],
            FIXED_KERNEL_STORAGE_PROFILE,
            bounds()
        )
        .unwrap_err(),
        FragmentSetError::DuplicateFragment
    );
    second.fragments[0].boot_id = "replaced-boot".into();
    assert_eq!(
        lower_local_fragment_set(
            &[&first.fragments[0], &second.fragments[0]],
            FIXED_KERNEL_STORAGE_PROFILE,
            bounds()
        )
        .unwrap_err(),
        FragmentSetError::DifferentHostBootOrGeneration
    );
}

#[test]
fn every_combined_storage_limit_is_enforced_at_its_exact_boundary() {
    let (first, second) = plans();
    let fragments = [&first.fragments[0], &second.fragments[0]];
    let set = lower_local_fragment_set(&fragments, FIXED_KERNEL_STORAGE_PROFILE, bounds()).unwrap();
    let exact = FragmentSetBounds {
        fragments: 2,
        nodes: set.nodes,
        cords: set.cords,
        queue_slots: set.queue_slots,
        value_bytes: set.value_bytes,
        sign_items: set.sign_items,
        sign_bytes: set.sign_bytes,
    };
    lower_local_fragment_set(&fragments, FIXED_KERNEL_STORAGE_PROFILE, exact).unwrap();
    for dimension in 0..7 {
        let mut insufficient = exact;
        match dimension {
            0 => insufficient.fragments -= 1,
            1 => insufficient.nodes -= 1,
            2 => insufficient.cords -= 1,
            3 => insufficient.queue_slots -= 1,
            4 => insufficient.value_bytes -= 1,
            5 => insufficient.sign_items -= 1,
            6 => insufficient.sign_bytes -= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            lower_local_fragment_set(&fragments, FIXED_KERNEL_STORAGE_PROFILE, insufficient)
                .unwrap_err(),
            FragmentSetError::Capacity,
            "storage dimension {dimension} must refuse one below the exact requirement"
        );
    }
}

#[test]
fn empty_workload_and_mixed_host_generations_refuse() {
    assert_eq!(
        lower_local_fragment_set(&[], FIXED_KERNEL_STORAGE_PROFILE, bounds()).unwrap_err(),
        FragmentSetError::Empty
    );
    let (first, second) = plans();
    for change_host in [true, false] {
        let mut changed = second.fragments[0].clone();
        if change_host {
            changed.host_id = "another-host".into();
        } else {
            changed.offer_generation.0 += 1;
        }
        assert_eq!(
            lower_local_fragment_set(
                &[&first.fragments[0], &changed],
                FIXED_KERNEL_STORAGE_PROFILE,
                bounds()
            )
            .unwrap_err(),
            FragmentSetError::DifferentHostBootOrGeneration
        );
    }
}
