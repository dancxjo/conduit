//! Real installed-operation preparation; this is not Body-wide execution proof.
use super::*;
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_plan_lowering::fragment_set::{lower_local_fragment_set, FragmentSetBounds};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE;

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
    let lowered =
        lower_local_fragment_set(&fragments, FIXED_KERNEL_STORAGE_PROFILE, bounds()).unwrap();
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
fn two_real_clock_partitions_complete_through_the_shared_kernel_installation() {
    use crate::installed_std::kernel_preparation::KernelTables;
    use conduit_kernel::scheduler::SchedulerStatus;
    use conduit_kernel::{HostOperationDisposition, HostOperationOutcome, HostedSignLog};

    let plans = plans();
    let snapshot = plans.clone();
    let fragments = [&plans[0].fragments[0], &plans[1].fragments[0]];
    let lowered =
        lower_local_fragment_set(&fragments, FIXED_KERNEL_STORAGE_PROFILE, bounds()).unwrap();
    let mut values = HostedValueStore::new(32, 64, 2048).unwrap();
    let mut drivers =
        core::array::from_fn(|_| OperationDriver::new(InstalledOperation::inactive()).unwrap());
    for (fragment, partition) in fragments.iter().zip(&lowered.partitions) {
        let play = conduit_core::bind_active_play(
            &fragment.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            1,
        );
        for (slot, driver) in prepare_operations(fragment, partition, &mut values, &play, None)
            .unwrap()
            .into_iter()
            .enumerate()
        {
            if partition
                .nodes
                .iter()
                .any(|node| usize::from(node.node.0) == slot)
            {
                drivers[slot] = driver;
            }
        }
    }
    let partitions: Vec<_> = lowered.partitions.iter().collect();
    let mut kernel = KernelTables::prepare(&partitions)
        .unwrap()
        .install(
            drivers,
            values,
            HostedSignLog::new(
                1024,
                1024 * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
            )
            .unwrap(),
        )
        .unwrap();
    let mut ticks = [Vec::with_capacity(4), Vec::with_capacity(4)];
    let mut waits = [0; 2];
    let mut completed = false;
    // Deterministic Host-operation completions, not wall-clock or OS proof.
    // All operation state machines and scheduling are the installed production path.
    for _ in 0..256 {
        while let Some(request) = kernel.next_host_request() {
            let partition = lowered
                .partitions
                .iter()
                .position(|partition| {
                    partition
                        .identity
                        .placement_for_node(request.node)
                        .is_some()
                })
                .unwrap();
            let operation = lowered.partitions[partition]
                .host_operations
                .iter()
                .find(|op| op.node == request.node && op.operation == request.operation)
                .unwrap();
            if operation.contract_id == conduit_core::wait_host_operation_requirement().contract_id
            {
                waits[partition] += 1;
            } else {
                assert_eq!(
                    operation.target_kind,
                    Some(conduit_core::kind_id(
                        conduit_std_offers::TICK_PRESENTATION_TARGET,
                    ))
                );
                ticks[partition].push(
                    conduit_time::decode_tick(kernel.host_value(request.input.value).unwrap())
                        .unwrap(),
                );
            }
            kernel
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .unwrap();
        }
        if kernel.step().unwrap() == SchedulerStatus::Complete {
            completed = true;
            break;
        }
    }
    assert!(
        completed,
        "both exact partitions must finish in the one kernel"
    );
    assert_eq!(ticks, [vec![0, 1, 2, 3], vec![0, 1, 2, 3]]);
    assert_eq!(waits, [4, 4]);
    assert_eq!(plans, snapshot);
    let result = KernelTables::prepare(&[partitions[0], partitions[0]]);
    assert!(matches!(result, Err(reason) if reason.contains("disjoint")));
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
