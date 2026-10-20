//! Real loopback transport retains kernel evidence from both planned Hosts.
use conduit::{execute_hosted_two_std_plot, execute_hosted_two_std_plot_with_evidence};
use conduit_kernel::KernelEventKind;

const SOURCE: &str = "plot hello_across {\n message: text/literal(\"hello across one Cord\")\n show: face/text(\"Message\")\n message >> show\n}\n";

#[test]
fn observed_two_host_execution_preserves_result_and_distinct_lifecycle_evidence() {
    let mut ordinary_output = Vec::new();
    let ordinary = execute_hosted_two_std_plot(SOURCE, &mut ordinary_output).unwrap();
    let mut observed_output = Vec::new();
    let (observed, evidence) =
        execute_hosted_two_std_plot_with_evidence(SOURCE, &mut observed_output).unwrap();
    assert_eq!(observed_output, ordinary_output);
    assert_eq!(observed.result, ordinary.result);
    assert_eq!(observed.source_document_id, ordinary.source_document_id);
    assert_eq!(observed.checked_plot_id, ordinary.checked_plot_id);
    assert_eq!(observed.expanded_plot_id, ordinary.expanded_plot_id);
    assert_eq!(observed.plan_id, ordinary.plan_id);
    assert_eq!(observed.line_id, ordinary.line_id);
    assert_eq!(observed.transferred_values, 1);
    assert_eq!(
        evidence
            .phases
            .iter()
            .map(|phase| phase.phase)
            .collect::<Vec<_>>(),
        [
            "source_parse_expand",
            "host_advertisements_context",
            "plan_select",
            "fragments_prepare",
            "line_and_evidence_retention",
            "output_and_execution_receipt",
        ]
    );
    assert_eq!(evidence.source.identity.plan_id, observed.plan_id);
    assert_eq!(evidence.sink.identity.plan_id, observed.plan_id);
    assert_eq!(
        evidence.source.identity.fragment_id,
        observed.source_fragment_id
    );
    assert_eq!(
        evidence.sink.identity.fragment_id,
        observed.sink_fragment_id
    );
    assert_ne!(
        evidence.source.identity.fragment_id,
        evidence.sink.identity.fragment_id
    );
    for host in [&evidence.source, &evidence.sink] {
        assert!(host.retention_gap.is_none());
        assert!(!host.events.is_empty());
        assert!(!host.remote_identities.is_empty());
        assert!(host
            .events
            .windows(2)
            .all(|events| events[0].sequence < events[1].sequence));
        for (sequence, remote) in &host.remote_identities {
            assert!(host.events.iter().any(|event| event.sequence == *sequence));
            assert!(host
                .identity
                .remote_endpoints
                .iter()
                .any(|(endpoint, _)| *endpoint == remote.endpoint));
        }
    }
    let position = |kind| {
        evidence
            .source
            .events
            .iter()
            .position(|event| event.kind == kind)
            .unwrap()
    };
    assert!(
        position(KernelEventKind::RemoteValueOffered)
            < position(KernelEventKind::RemoteValueAccepted)
    );
    assert!(
        position(KernelEventKind::RemoteValueAccepted)
            < position(KernelEventKind::RemoteValueDelivered)
    );
    for kind in [
        KernelEventKind::RemoteValueOffered,
        KernelEventKind::RemoteValueAccepted,
        KernelEventKind::RemoteValueDelivered,
    ] {
        let event = evidence
            .source
            .events
            .iter()
            .find(|event| event.kind == kind)
            .unwrap();
        assert!(evidence
            .source
            .remote_identities
            .iter()
            .any(|(sequence, _)| *sequence == event.sequence));
    }
    assert!(evidence
        .sink
        .events
        .iter()
        .any(|event| event.kind == KernelEventKind::HostCallCompleted));
    assert!(evidence
        .sink
        .events
        .iter()
        .any(|event| event.kind == KernelEventKind::RemoteInputClosed));
}

#[test]
fn measured_payload_cases_fit_the_admitted_real_line() {
    for length in [8, 64, 65, 256] {
        let payload = "x".repeat(length);
        let source = format!("plot payload_case {{\n message: text/literal(\"{payload}\")\n show: face/text(\"Message\")\n message >> show\n}}\n");
        let mut output = Vec::new();
        let (execution, evidence) =
            execute_hosted_two_std_plot_with_evidence(&source, &mut output).unwrap();
        assert_eq!(execution.result, payload);
        assert_eq!(output, format!("{payload}\n").as_bytes());
        assert!(evidence.source.retention_gap.is_none() && evidence.sink.retention_gap.is_none());
        assert_eq!(evidence.phases.len(), 6);
    }
}

#[test]
fn payload_above_the_text_contract_is_refused_before_transport() {
    let payload = "x".repeat(257);
    let source = format!("plot oversized {{\n message: text/literal(\"{payload}\")\n show: face/text(\"Message\")\n message >> show\n}}\n");
    let mut output = Vec::new();
    assert!(execute_hosted_two_std_plot_with_evidence(&source, &mut output).is_err());
    assert!(output.is_empty());
}

#[test]
fn persistent_tick_flow_retains_all_values_and_both_kernel_lifecycles() {
    let count = 64;
    let mut output = Vec::new();
    let (execution, evidence) =
        conduit::execute_hosted_two_std_tick_flow_with_evidence(count, &mut output).unwrap();
    assert_eq!(execution.transferred_values, u32::from(count));
    assert_eq!(execution.result, "63");
    let expected: String = (0..count).map(|n| format!("{n}\n")).collect();
    assert_eq!(String::from_utf8(output).unwrap(), expected);
    for host in [&evidence.source, &evidence.sink] {
        assert!(host.retention_gap.is_none());
        assert_eq!(host.identity.plan_id, execution.plan_id);
    }
    for wire_sequence in 1..=u64::from(count) {
        let mut kinds = Vec::new();
        for event in &evidence.source.events {
            if !matches!(
                event.kind,
                KernelEventKind::RemoteValueOffered
                    | KernelEventKind::RemoteValueAccepted
                    | KernelEventKind::RemoteValueDelivered
            ) {
                continue;
            }
            let identity = evidence
                .source
                .remote_identities
                .iter()
                .find(|(sequence, _)| *sequence == event.sequence)
                .expect("every retained event keeps its exact remote correlation")
                .1;
            if identity.sequence == wire_sequence {
                kinds.push(event.kind);
            }
        }
        assert_eq!(
            kinds,
            [
                KernelEventKind::RemoteValueOffered,
                KernelEventKind::RemoteValueAccepted,
                KernelEventKind::RemoteValueDelivered
            ]
        );
    }
}

#[test]
fn persistent_tick_flow_count_refuses_outside_its_preplay_bound() {
    for count in [0, 65] {
        let mut output = Vec::new();
        assert!(
            conduit::execute_hosted_two_std_tick_flow_with_evidence(count, &mut output).is_err()
        );
        assert!(output.is_empty());
    }
}
