//! Deterministic execution conformance; debug timing values are discarded.
use super::*;

#[test]
fn ordinary_checked_pipelines_keep_equal_work_and_exact_boot_custody() -> Result<(), Box<dyn Error>>
{
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    for gears in [1, 8, 32] {
        let boot = format!("boot/runtime-performance-test/{gears}");
        let mut prepared = prepare(gears, &source(gears), &boot)?;
        let mut collector = Collector::new();
        let (report, full_call) = timing::measure(|| {
            prepared.host.run_external_plot_sequence_to(
                prepared.plan.fragments[0].clone(),
                &prepared.inputs,
                &mut collector,
                &mut std::io::sink(),
                &mut NoTimer,
            )
        });
        let report = report?;
        let window = collector.allocation_window()?;
        assert!(window.allocations <= full_call.allocator.allocations);
        assert!(window.reallocations <= full_call.allocator.reallocations);
        assert!(window.requested_bytes <= full_call.allocator.requested_bytes);
        assert_eq!(collector.values.len(), 64);
        for (index, value) in collector.values.iter().enumerate() {
            assert_eq!(value.bytes, (index as u64 + 32).to_le_bytes());
            assert_eq!(value.sequence, index as u64);
            assert_eq!(value.value_kind, conduit_core::kind_id("value/u64"));
        }
        for bytes in [vec![0, 1], u64::MAX.to_le_bytes().to_vec()] {
            let mut invalid = prepared.inputs[..1].to_vec();
            invalid[0].bytes = bytes;
            let mut refused_outputs = Collector::new();
            assert!(native(&invalid, &mut Vec::new()).is_err());
            assert!(prepared
                .host
                .run_external_plot_sequence_to(
                    prepared.plan.fragments[0].clone(),
                    &invalid,
                    &mut refused_outputs,
                    &mut std::io::sink(),
                    &mut NoTimer,
                )
                .is_err());
            assert!(refused_outputs.values.is_empty());
        }
        let mut native_outputs = Vec::with_capacity(64);
        native(&prepared.inputs, &mut native_outputs)?;
        let ordered_native = native_outputs.iter().flatten().copied().collect::<Vec<_>>();
        assert_eq!(
            validate(&collector.values)?,
            artifact::digest(&ordered_native)
        );
        assert_eq!(report.external_fore_deliveries, collector.values);
        let kernel = report.kernel.ok_or("normal kernel receipt absent")?;
        assert_eq!(kernel.active_play.boot_id.as_str(), boot);
        assert_eq!(kernel.active_play.plan_id, prepared.plan.plan_id);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert_eq!(
            kernel
                .kernel_sign
                .iter()
                .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
                .count(),
            64
        );
        // A canonical frame can have the right bytes and still violate the
        // exact execution boundary; none of these substitutions are admissible.
        for alteration in 0..4 {
            let mut changed = collector.values.clone();
            match alteration {
                0 => changed[0].value_kind = conduit_core::kind_id("value/i64"),
                1 => changed[0].front_port_id = "foreign-output".into(),
                2 => changed[0].sequence = 1,
                _ => changed.swap(0, 1),
            }
            assert!(validate(&changed).is_err());
        }
    }
    Ok(())
}

#[test]
fn absent_acknowledgements_do_not_fabricate_zero_allocation_measurements() {
    let collector = Collector::new();
    assert!(collector.allocation_window().is_err());
}

#[test]
fn installed_local_pipeline_preserves_outputs_and_refuses_bad_values() -> Result<(), Box<dyn Error>>
{
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    for gears in [1, 8, 32] {
        let authored = source(gears);
        let mut prepared = prepare_profile(gears, &authored, "boot/local-pipeline", true)?;
        let reference = prepare(gears, &authored, "boot/local-pipeline")?;
        assert_eq!(
            prepared.plan.checked_plot_id,
            reference.plan.checked_plot_id
        );
        for (local, hosted) in prepared.plan.fragments[0]
            .placements
            .iter()
            .zip(&reference.plan.fragments[0].placements)
        {
            assert_eq!(local.gear_id, hosted.gear_id);
            assert_eq!(local.kind_id, hosted.kind_id);
            assert_eq!(local.inputs, hosted.inputs);
            assert_eq!(local.outputs, hosted.outputs);
            assert_eq!(local.configuration, hosted.configuration);
            assert_eq!(local.semantic_contract, hosted.semantic_contract);
        }
        assert_eq!(prepared.plan.fragments[0].placements.len(), gears);
        assert!(prepared.plan.fragments[0]
            .placements
            .iter()
            .all(|p| p.host_calls.is_empty()));
        let mut collector = Collector::new();
        let report = prepared.host.run_external_plot_sequence_to(
            prepared.plan.fragments[0].clone(),
            &prepared.inputs,
            &mut collector,
            &mut std::io::sink(),
            &mut NoTimer,
        )?;
        assert_eq!(collector.values.len(), ITEMS);
        let mut oracle = Vec::with_capacity(ITEMS);
        native(&prepared.inputs, &mut oracle)?;
        assert_eq!(
            validate(&collector.values)?,
            artifact::digest(&oracle.iter().flatten().copied().collect::<Vec<_>>())
        );
        let kernel = report.kernel.ok_or("kernel receipt absent")?;
        assert_eq!(kernel.active_play.plan_id, prepared.plan.plan_id);
        for bytes in [vec![0, 1], u64::MAX.to_le_bytes().to_vec()] {
            let mut invalid = prepared.inputs[..1].to_vec();
            invalid[0].bytes = bytes;
            let mut refused = Collector::new();
            assert!(prepared
                .host
                .run_external_plot_sequence_to(
                    prepared.plan.fragments[0].clone(),
                    &invalid,
                    &mut refused,
                    &mut std::io::sink(),
                    &mut NoTimer,
                )
                .is_err());
            assert!(refused.values.is_empty());
        }
    }
    Ok(())
}

#[test]
fn fused_pipeline_uses_one_region_with_equal_outputs() -> Result<(), Box<dyn Error>> {
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    assert_eq!(
        source(32),
        include_str!("../../fixtures/pure-chain-32.conduit")
    );
    for gears in [1, 8, 32] {
        let mut prepared = prepare_mode(gears, &source(gears), "boot/fused-pipeline", true, true)?;
        assert_eq!(
            prepared.plan.fragments[0].execution_fusions.len(),
            usize::from(gears > 1)
        );
        let mut collector = Collector::new();
        let report = prepared.host.run_external_plot_sequence_to(
            prepared.plan.fragments[0].clone(),
            &prepared.inputs,
            &mut collector,
            &mut std::io::sink(),
            &mut NoTimer,
        )?;
        assert_eq!(collector.values.len(), ITEMS);
        for (i, value) in collector.values.iter().enumerate() {
            assert_eq!(value.bytes, (i as u64 + ADDS as u64).to_le_bytes());
        }
        let kernel = report.kernel.ok_or("kernel receipt missing")?;
        let mut reference = prepare_profile(gears, &source(gears), "boot/fused-reference", true)?;
        let mut reference_outputs = Collector::new();
        let reference_report = reference.host.run_external_plot_sequence_to(
            reference.plan.fragments[0].clone(),
            &reference.inputs,
            &mut reference_outputs,
            &mut std::io::sink(),
            &mut NoTimer,
        )?;
        let reference_decisions = reference_report
            .kernel
            .ok_or("reference kernel receipt missing")?
            .decisions;
        assert_eq!(
            validate(&collector.values)?,
            validate(&reference_outputs.values)?
        );
        if gears > 1 {
            assert!(kernel.decisions < reference_decisions);
        } else {
            assert_eq!(kernel.decisions, reference_decisions);
        }
        assert_eq!(
            kernel
                .kernel_sign
                .iter()
                .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
                .count(),
            ITEMS
        );
    }
    Ok(())
}

#[test]
fn ordinary_planner_can_keep_unfused_cost_or_required_observation() -> Result<(), Box<dyn Error>> {
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    let authored = source(32);
    let syntax = parse_syntax_document(&authored);
    let checked =
        check_syntax_document(&syntax, &StartupCatalog::new()).map_err(|e| format!("{e:?}"))?;
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "baseline", &ProfileCatalog::new())
            .map_err(|e| format!("{e:?}"))?;
    let local = prepare_profile(32, &authored, "boot/cost-refusal", true)?;
    let hosts = [local.host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts)?;
    let expensive = fusion::select(&authoring, &hosts, &placements, 1000, &[])?;
    assert_eq!(expensive.selected_candidate_id, "unfused");
    let cord = &authoring.expanded.connections[0];
    let observed = fusion::select(
        &authoring,
        &hosts,
        &placements,
        1,
        &[conduit_planner::FusionBoundary {
            source_gear_id: cord.source_gear_id.clone(),
            sink_gear_id: cord.sink_gear_id.clone(),
            requires_observation: true,
            requires_authority: false,
        }],
    )?;
    assert_eq!(observed.selected_candidate_id, "unfused");
    assert!(observed.considered.iter().any(|candidate|matches!(&candidate.disposition,
        conduit_planner::CandidatePlacementDisposition::Rejected(reason) if reason.contains("observation or authority"))));
    let mut independent_fore = authoring.clone();
    independent_fore.output_bindings[0].gear_id =
        independent_fore.input_bindings[0].gear_id.clone();
    let refusal =
        conduit_std_host::pure_chain_fusion::pure_chain_fusion_offer(&independent_fore, &hosts[0])
            .unwrap_err();
    assert!(refusal.contains("independent Fore or observation"));
    let hosted = prepare(32, &authored, "boot/host-call-refusal")?;
    assert!(
        conduit_std_host::pure_chain_fusion::pure_chain_fusion_offer(
            &authoring,
            hosted.host.advertisement()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn checked_fused_and_local_refusals_name_every_original_gear_and_span() -> Result<(), Box<dyn Error>>
{
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    let authored = source(32);
    for fused in [false, true] {
        let mut prepared = prepare_mode(32, &authored, "boot/exact-failure", true, fused)?;
        let fragment = &prepared.plan.fragments[0];
        let mut order = Vec::new();
        let mut current = fragment
            .placements
            .iter()
            .find(|p| {
                !fragment
                    .connections
                    .iter()
                    .any(|c| c.sink_placement_id == p.placement_id)
            })
            .ok_or("chain entry absent")?;
        loop {
            order.push(current.gear_id.clone());
            let Some(next) = fragment
                .connections
                .iter()
                .find(|c| c.source_placement_id == current.placement_id)
            else {
                break;
            };
            current = fragment
                .placements
                .iter()
                .find(|p| p.placement_id == next.sink_placement_id)
                .ok_or("chain sink absent")?;
        }
        assert_eq!(order.len(), 32);
        for (index, gear) in order.iter().enumerate() {
            let mut invalid = prepared.inputs[..1].to_vec();
            invalid[0].bytes = (u64::MAX - index as u64).to_le_bytes().to_vec();
            let mut outputs = Collector::new();
            let reason = prepared
                .host
                .run_external_plot_sequence_to(
                    fragment.clone(),
                    &invalid,
                    &mut outputs,
                    &mut std::io::sink(),
                    &mut NoTimer,
                )
                .unwrap_err();
            assert!(
                reason.contains(&format!("logical-gear={}", gear.as_str())),
                "{index}: {reason}"
            );
            assert!(reason.contains("source-span=Some"), "{reason}");
            assert!(outputs.values.is_empty());
        }
    }
    Ok(())
}

#[test]
fn reviewed_physical_profile_refuses_other_numeric_representations() -> Result<(), Box<dyn Error>> {
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    let source = source(8).replace("U64", "Scalar");
    let refusal = prepare_mode(8, &source, "boot/unsupported-scalar-fusion", true, true)
        .err()
        .ok_or("unsupported Scalar fusion was admitted")?
        .to_string();
    assert!(
        refusal.contains("supports exact U64 input/output only"),
        "{refusal}"
    );
    Ok(())
}
