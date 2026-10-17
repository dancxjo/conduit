use super::*;
#[test]
fn ordinary_overlapping_streams_preserve_order_fanout_pressure_and_payloads(
) -> Result<(), Box<dyn Error>> {
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    for (index, case) in cases().into_iter().enumerate() {
        let mut prepared = prepare_case(case, &format!("boot/stream-conformance/{index}"), 128)?;
        let mut collector = StreamCollector::new(case, 128);
        let (result, _) = timing::measure(|| {
            collector.begin();
            prepared.host.run_external_plot_stream_controlled_to(
                prepared.plan.fragments[0].clone(),
                &prepared.inputs,
                &mut collector,
                &mut std::io::sink(),
                &mut NoTimer,
                &RunControl::default(),
                ExternalForeStreamConfig {
                    admission_batch: case.batch,
                },
            )
        });
        let report = result.map_err(|error| {
            format!(
                "{}: {error}; Fore={:?}",
                case.id(),
                prepared.plan.fragments[0].fore_ports[0]
            )
        })?;
        let kernel = report.kernel.ok_or("kernel missing")?;
        let ingress = kernel.external_fore_stream.unwrap();
        assert_eq!(ingress.accepted, 128);
        assert_eq!(ingress.attempts, u32::from(ingress.accepted) + ingress.full);
        if case.queue == 1 && case.batch == 8 {
            assert!(ingress.full > 0);
        }
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
            128 * case.branches
        );
        let observed = collector.report()?;
        for library in [false, true] {
            let values = native(case, &prepared.inputs, library)?;
            assert_eq!(
                observed["ordered_output_sha256"],
                observation::validate_native(case, 128, &values)?
            );
        }
    }
    Ok(())
}
#[test]
fn malformed_overflow_cancel_and_unplanned_fore_never_become_successful_outputs(
) -> Result<(), Box<dyn Error>> {
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    let case = cases()[0];
    for mode in 0..4 {
        let mut prepared = prepare_case(case, &format!("boot/stream-negative/{mode}"), 8)?;
        let control = RunControl::default();
        match mode {
            0 => prepared.inputs[0].bytes = vec![0, 1],
            1 => prepared.inputs[0].bytes = u64::MAX.to_le_bytes().to_vec(),
            2 => control
                .request_stop(conduit_std_host::RunControlRequestId::new("stop")?)
                .unwrap(),
            _ => prepared.inputs[0].front_port_id = "other".into(),
        }
        let mut collector = StreamCollector::new(case, 8);
        let (result, _) = timing::measure(|| {
            collector.begin();
            prepared.host.run_external_plot_stream_controlled_to(
                prepared.plan.fragments[0].clone(),
                &prepared.inputs,
                &mut collector,
                &mut std::io::sink(),
                &mut NoTimer,
                &control,
                ExternalForeStreamConfig { admission_batch: 8 },
            )
        });
        if mode == 2 {
            let report = result?;
            assert!(matches!(
                report.observations.last().map(|o| &o.kind),
                Some(conduit_core::ObservationKind::PlanTerminal {
                    disposition: conduit_core::TerminalDisposition::Cancelled { .. }
                })
            ));
        } else {
            assert!(result.is_err());
        }
        assert!(collector.report().is_err());
    }
    Ok(())
}

#[test]
fn oversized_text_keeps_the_existing_checked_256_byte_refusal() -> Result<(), Box<dyn Error>> {
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    let case = cases()
        .into_iter()
        .find(|c| c.text && c.payload_bytes == 256)
        .unwrap();
    let mut prepared = prepare_case(case, "boot/stream-text-refusal", 8)?;
    prepared.inputs[0].bytes = vec![b'x'; 512];
    let mut collector = StreamCollector::new(case, 8);
    let (result, _) = timing::measure(|| {
        collector.begin();
        prepared.host.run_external_plot_stream_controlled_to(
            prepared.plan.fragments[0].clone(),
            &prepared.inputs,
            &mut collector,
            &mut std::io::sink(),
            &mut NoTimer,
            &RunControl::default(),
            ExternalForeStreamConfig { admission_batch: 8 },
        )
    });
    assert!(result.unwrap_err().contains("sealed value contract"));
    assert!(collector.report().is_err());
    Ok(())
}
