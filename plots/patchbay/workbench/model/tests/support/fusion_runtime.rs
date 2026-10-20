//! Actual installed execution and source-linked failure/refusal controls.
use super::*;

pub(super) fn assert_actual_failure_custody(
    fused: &Plan,
    advertisement: &HostAdvertisement,
) -> Plan {
    use conduit_std_host::{
        ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, TimerAdapter,
    };
    struct NoTimer;
    impl TimerAdapter for NoTimer {
        fn wait(&mut self, _: std::time::Duration) {}
    }
    #[derive(Default)]
    struct Outputs(Vec<ExternalForeDelivery>);
    impl ExternalForeOutputAdapter for Outputs {
        fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
            self.0.push(output);
            Ok(())
        }
    }
    let mut fragments = fused.fragments.clone();
    for fragment in &mut fragments {
        fragment.execution_fusions.clear();
    }
    let unfused = seal_plan_with_realization_backs_and_completion(
        PlotIdentity {
            source_document_id: fused.source_document_id.clone(),
            checked_plot_id: fused.checked_plot_id.clone(),
            expanded_plot_id: fused.expanded_plot_id.clone(),
        },
        fused.completion_policy,
        fused.realization_backs.clone(),
        fragments,
    );
    assert!(verify_plan(&unfused));
    let mut host = StdHost::from_advertisement(advertisement.clone()).unwrap();
    let mut unavailable = fused.fragments.clone();
    unavailable[0].execution_fusions[0].artifact_id = "not-installed/fusion-artifact".into();
    let unavailable = seal_plan_with_realization_backs_and_completion(
        PlotIdentity {
            source_document_id: fused.source_document_id.clone(),
            checked_plot_id: fused.checked_plot_id.clone(),
            expanded_plot_id: fused.expanded_plot_id.clone(),
        },
        fused.completion_policy,
        fused.realization_backs.clone(),
        unavailable,
    );
    assert!(verify_plan(&unavailable));
    let refusal = host
        .run_external_plot_to(
            unavailable.fragments[0].clone(),
            &[],
            &mut Outputs::default(),
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap_err();
    assert!(
        refusal.contains("unsupported std physical fusion"),
        "{refusal}"
    );
    assert!(refusal.contains(unavailable.plan_id.as_str()), "{refusal}");
    assert!(
        refusal.contains(unavailable.source_document_id.as_str()),
        "{refusal}"
    );

    for plan in [&unfused, fused] {
        let fragment = &plan.fragments[0];
        let mut failing_id = fragment
            .fore_ports
            .iter()
            .find(|p| p.direction == PortDirection::Input)
            .unwrap()
            .placement_id
            .clone();
        for _ in 0..2 {
            failing_id = fragment
                .connections
                .iter()
                .find(|cord| cord.source_placement_id == failing_id)
                .unwrap()
                .sink_placement_id
                .clone();
        }
        let expected = fragment
            .placements
            .iter()
            .find(|p| p.placement_id == failing_id)
            .unwrap();
        let mut outputs = Outputs::default();
        let input = |value: u64| {
            [ExternalForeInput {
                front_port_id: "input".into(),
                track: ConnectionTrack::Payload,
                bytes: value.to_le_bytes().to_vec(),
            }]
        };
        host.run_external_plot_to(
            fragment.clone(),
            &input(1),
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
        assert_eq!(outputs.0.len(), 1);
        assert_eq!(&*outputs.0[0].bytes, &33_u64.to_le_bytes());
        outputs.0.clear();
        let error = host
            .run_external_plot_to(
                fragment.clone(),
                &input(u64::MAX - 2),
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap_err();
        assert!(outputs.0.is_empty());
        assert!(
            error.contains(&format!("logical-gear={}", expected.gear_id.as_str())),
            "{error}"
        );
        assert!(
            error.contains(&format!("placement={}", expected.placement_id.as_str())),
            "{error}"
        );
        assert!(error.contains(fused.source_document_id.as_str()));
        assert!(error.contains(&format!("source-span={:?}", expected.source_span)));
    }
    unfused
}
