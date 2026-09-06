//! Real installed-operation preparation; this is not Body-wide execution proof.
use super::*;
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_plan_lowering::fragment_set::{lower_local_fragment_set, FragmentSetBounds};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE;

fn plans() -> [conduit_core::Plan; 2] {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    let host = crate::StdHost::new();
    let source = include_str!("../../../../forms/clock/main.conduit");
    [source.to_string(), source.replace("1s", "2s")].map(|source| {
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let expanded = expand_canonical_form(&checked, "clock-demo", &profile).unwrap();
        host.plan_expanded_local(&expanded).unwrap()
    })
}

#[test]
fn real_partition_operations_use_global_slots_and_original_placement_identity() {
    let plans = plans();
    let fragments = [&plans[0].fragments[0], &plans[1].fragments[0]];
    let lowered = lower_local_fragment_set(
        &fragments,
        FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 2,
            nodes: 4,
            cords: 2,
            queue_slots: 16,
            value_bytes: 1024,
            sign_items: 1024,
            sign_bytes: 65536,
        },
    )
    .unwrap();
    let mut values = HostedValueStore::new(32, 64, 2048).unwrap();
    for (index, fragment) in fragments.iter().enumerate() {
        let play = conduit_core::bind_active_play(
            &fragment.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            1,
        );
        let drivers = prepare_operations(
            fragment,
            &lowered.partitions[index],
            &mut values,
            &play,
            None,
        )
        .unwrap();
        let offset = index * 2;
        assert!(matches!(
            drivers[offset].operation(),
            InstalledOperation::Tick(_)
        ));
        assert!(matches!(
            drivers[offset + 1].operation(),
            InstalledOperation::TickPresentation(_)
        ));
        for (slot, driver) in drivers.iter().enumerate() {
            if slot < offset || slot >= offset + 2 {
                assert!(matches!(driver.operation(), InstalledOperation::Inactive));
            }
        }
    }
}

#[test]
fn wrong_partition_and_out_of_range_nodes_refuse_before_value_preparation() {
    let plans = plans();
    let fragment = &plans[0].fragments[0];
    let mut lowered = lower_fragment_with_continuity(fragment, false).unwrap();
    let play =
        conduit_core::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    let mut values = HostedValueStore::new(32, 64, 2048).unwrap();
    let result = prepare_operations(&plans[1].fragments[0], &lowered, &mut values, &play, None);
    assert!(matches!(result, Err(reason) if reason.contains("exact partition")));
    lowered.nodes[0].node.0 = MAX_NODES as u16;
    let result = prepare_operations(fragment, &lowered, &mut values, &play, None);
    assert!(matches!(result, Err(reason) if reason.contains("driver capacity")));
    lowered.nodes[0].node = lowered.nodes[1].node;
    let result = prepare_operations(fragment, &lowered, &mut values, &play, None);
    assert!(matches!(result, Err(reason) if reason.contains("duplicate")));
}
