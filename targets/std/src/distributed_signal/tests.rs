use super::*;

#[test]
fn unchanged_form_prepares_exact_independent_remote_fragments() {
    let canonical_source = include_str!("../../../../forms/signal-demo/main.conduit");
    for realization_fact in ["std", "browser", "websocket", "host", "line"] {
        assert!(!canonical_source
            .to_ascii_lowercase()
            .contains(realization_fact));
    }
    let source = DistributedSource::prepare().expect("source prepares");
    let exact = exact_distributed_signal_plan().expect("distributed plan resolves");
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == exact.sink_advertisement.host_id)
        .expect("sink fragment");
    let lowered = lower_plan_fragment(sink).expect("sink lowers");
    assert_eq!(source.fragment().placements.len(), 1);
    assert_eq!(sink.placements.len(), 1);
    assert_ne!(source.fragment().host_id, sink.host_id);
    assert_eq!(
        source.fragment().connections[0]
            .selected_line
            .as_ref()
            .unwrap()
            .binding
            .base,
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1")
    );
    assert_eq!(lowered.remote_endpoints.len(), 1);
    assert_eq!(
        lowered.remote_endpoints[0].direction,
        RemoteCordDirection::Ingress
    );
    assert_eq!(source.binding().plan_id, sink.plan_id);
    assert_eq!(source.binding().sink_fragment_id, sink.fragment_id);
    assert_eq!(source.binding().limits.maximum_in_flight_items, 1);
    assert_eq!(
        source.binding().limits.maximum_payload_bytes,
        SIGNAL_ENCODED_LEN
    );
    assert_eq!(
        source.binding().limits.maximum_buffered_bytes,
        SIGNAL_ENCODED_LEN
    );
}

#[test]
fn missing_and_stale_observed_links_fail_planning() {
    let source = distributed_std_source_advertisement();
    let sink = distributed_browser_sink_advertisement();
    let form = conduit_form::parse_with_startup(
        include_str!("../../../../proof/fixtures/forms/signal-demo.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .unwrap();
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("pulse"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("pulse-1"),
                },
            ),
            (
                GearId::from("show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("dom-show-1"),
                },
            ),
        ]),
    };
    assert!(plan_with_line_offers(
        &form,
        &[source.clone(), sink.clone()],
        &placements,
        &[BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1"
        )],
        1,
        SIGNAL_ENCODED_LEN,
        &[],
    )
    .is_err());
    let mut stale_source = distributed_websocket_line_offer();
    stale_source.binding.source.boot_id = conduit_core::BootId::from("stale-source");
    assert!(plan_with_line_offers(
        &form,
        &[source.clone(), sink.clone()],
        &placements,
        &[BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1"
        )],
        1,
        SIGNAL_ENCODED_LEN,
        &[stale_source],
    )
    .is_err());
    let mut stale_sink = distributed_websocket_line_offer();
    stale_sink.binding.sink.boot_id = conduit_core::BootId::from("stale-browser");
    assert!(plan_with_line_offers(
        &form,
        &[source, sink],
        &placements,
        &[BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1"
        )],
        1,
        SIGNAL_ENCODED_LEN,
        &[stale_sink],
    )
    .is_err());
}

#[test]
fn source_cancellation_releases_in_flight_values_and_rejects_late_acknowledgement() {
    let mut source = DistributedSource::prepare().expect("source prepares");
    let binding = source.binding.clone();
    source
        .session
        .admit_outbound(binding.hello_frame())
        .unwrap();
    source.session.admit_inbound(binding.hello_frame()).unwrap();
    source
        .session
        .admit_outbound(binding.frame(SessionMessage::Ready))
        .unwrap();
    source
        .session
        .admit_inbound(binding.frame(SessionMessage::Ready))
        .unwrap();
    let (sequence, payload) = source.next_offer().unwrap().unwrap();
    source
        .session
        .admit_outbound(binding.frame(SessionMessage::Offered {
            sequence,
            payload: &payload,
        }))
        .unwrap();
    source.scheduler.cancel().unwrap();
    source
        .session
        .admit_outbound(binding.frame(SessionMessage::Cancelled { code: 51 }))
        .unwrap();
    source
        .session
        .admit_outbound(binding.frame(SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Cancelled,
            final_sequence: 0,
        }))
        .unwrap();
    assert_eq!(
        source
            .session
            .admit_inbound(binding.frame(SessionMessage::Accepted { sequence })),
        Err(conduit_wire::WireError::InvalidState)
    );
    assert_eq!(source.scheduler.values().used_items(), 0);
    assert_eq!(source.capacity_seal(), source.seal);
}
