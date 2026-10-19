//! Owned and synchronous borrowed Flow callbacks through the same checked pipeline.
use super::*;

#[test]
fn borrowed_flow_acknowledgement_avoids_owned_clone_with_same_kernel_evidence(
) -> Result<(), Box<dyn Error>> {
    let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
    struct SynchronousSink {
        count: usize,
        first: Option<crate::allocation::Counts>,
        last: Option<crate::allocation::Counts>,
        refuse: bool,
        stop: Option<conduit_std_host::RunControl>,
    }
    impl ExternalForeOutputAdapter for SynchronousSink {
        fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
            panic!("Flow must use the borrowed callback")
        }
        fn deliver_flow(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
            if self.refuse {
                return Err("synchronous sink refused".into());
            }
            assert_eq!(value.sequence, self.count as u64);
            assert_eq!(value.bytes, (self.count as u64 + 32).to_le_bytes());
            assert_eq!(value.front_port_id.as_str(), "output");
            assert_eq!(value.value_kind.as_str(), "value/u64");
            if self.count == 0 {
                self.first = crate::allocation::snapshot();
            }
            if self.count == ITEMS - 1 {
                self.last = crate::allocation::snapshot();
            }
            self.count += 1;
            if let Some(control) = &self.stop {
                control
                    .request_stop(
                        conduit_std_host::RunControlRequestId::new("borrowed-stop").unwrap(),
                    )
                    .map_err(|error| format!("request borrowed stop: {error:?}"))?;
            }
            Ok(())
        }
    }
    for gears in [1, 8, 32] {
        let mut prepared = prepare(gears, &source(gears), "boot/borrowed-flow")?;
        let fragment = prepared.plan.fragments[0].clone();
        let mut owned = Collector::new();
        let (reference, _) = timing::measure(|| {
            prepared.host.run_external_plot_sequence_to(
                fragment.clone(),
                &prepared.inputs,
                &mut owned,
                &mut std::io::sink(),
                &mut NoTimer,
            )
        });
        let reference = reference?;
        let mut sink = SynchronousSink {
            count: 0,
            first: None,
            last: None,
            refuse: false,
            stop: None,
        };
        let (report, _) = timing::measure(|| {
            prepared.host.run_external_plot_sequence_to(
                fragment.clone(),
                &prepared.inputs,
                &mut sink,
                &mut std::io::sink(),
                &mut NoTimer,
            )
        });
        let report = report?;
        assert_eq!(sink.count, ITEMS);
        assert_eq!(
            report.external_fore_deliveries,
            reference.external_fore_deliveries
        );
        assert_eq!(
            report.kernel.as_ref().unwrap().kernel_sign,
            reference.kernel.as_ref().unwrap().kernel_sign,
        );
        let owned_window = owned.allocation_window()?;
        let borrowed_window = sink
            .first
            .as_ref()
            .unwrap()
            .interval_to(sink.last.as_ref().unwrap())
            .unwrap();
        assert_eq!(owned_window.allocations, ITEMS as u64 - 1);
        assert_eq!(owned_window.reallocations, 0);
        assert_eq!(owned_window.allocations, borrowed_window.allocations);
        assert_eq!(
            owned_window.requested_bytes,
            borrowed_window.requested_bytes
        );
        println!("Flow allocation window gears={gears}: owned={owned_window:?}, borrowed={borrowed_window:?}");
        sink.refuse = true;
        assert!(prepared
            .host
            .run_external_plot_sequence_to(
                fragment.clone(),
                &prepared.inputs,
                &mut sink,
                &mut std::io::sink(),
                &mut NoTimer,
            )
            .unwrap_err()
            .contains("synchronous sink refused"));
        sink.refuse = false;
        sink.count = 0;
        let control = conduit_std_host::RunControl::default();
        sink.stop = Some(control.clone());
        let cancelled = prepared.host.run_external_plot_sequence_controlled_to(
            fragment,
            &prepared.inputs,
            &mut sink,
            &mut std::io::sink(),
            &mut NoTimer,
            &control,
        )?;
        assert_eq!(sink.count, 1);
        assert_eq!(cancelled.external_fore_deliveries.len(), 1);
        assert_eq!(cancelled.control_receipts.len(), 1);
        assert!(matches!(
            cancelled.observations.last().map(|event| &event.kind),
            Some(conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Cancelled { .. }
            })
        ));
    }
    Ok(())
}
