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
        let report = prepared.host.run_external_plot_sequence_to(
            prepared.plan.fragments[0].clone(),
            &prepared.inputs,
            &mut collector,
            &mut std::io::sink(),
            &mut NoTimer,
        )?;
        assert_eq!(collector.values.len(), 64);
        for (index, value) in collector.values.iter().enumerate() {
            assert_eq!(value.bytes, (index as u64 + 32).to_le_bytes());
            assert_eq!(value.sequence, index as u64);
            assert_eq!(value.value_kind, conduit_core::kind_id("value/u64"));
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
