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
