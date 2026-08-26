//! Tests for `DistributedToggleSource` — adapter boundary and fragment preparation proofs.
//!
//! These tests are declared via `#[path = ...]` inside `source.rs` so they
//! retain access to private fields and methods through `super::*`.

use super::super::plan::exact_distributed_toggle_plan;
use super::*;
use conduit_core::{CapabilityId, ConnectionBase, GearId};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_runtime::lowering::RemoteCordDirection;
use conduit_signal::signal_profile_catalog;
use conduit_signal_conformance::{
    distributed_toggle_browser_sink_advertisement, distributed_toggle_std_source_advertisement,
};
use std::collections::BTreeMap;

#[test]
fn unchanged_toggle_form_prepares_exact_independent_remote_fragments() {
    let source = DistributedToggleSource::prepare().expect("toggle source prepares");
    let exact = exact_distributed_toggle_plan().expect("distributed toggle plan resolves");
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == exact.sink_advertisement.host_id)
        .expect("sink fragment");
    let lowered = lower_plan_fragment(sink).expect("sink lowers");
    // Source fragment has 2 nodes (trigger + toggle)
    assert_eq!(source.fragment().placements.len(), 2);
    // Sink fragment has 1 node (show)
    assert_eq!(sink.placements.len(), 1);
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
        conduit_signal::TRIGGER_ENCODED_LEN
    );
}

#[test]
fn missing_link_binding_fails_toggle_planning() {
    let source = distributed_toggle_std_source_advertisement();
    let sink = distributed_toggle_browser_sink_advertisement();
    let form = conduit_form::parse_with_startup(
        include_str!("../../../../fixtures/forms/remote-toggle.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .unwrap();
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("trigger"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("trigger-1"),
                },
            ),
            (
                GearId::from("toggle"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-1"),
                },
            ),
            (
                GearId::from("show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-dom-show-1"),
                },
            ),
        ]),
    };
    assert!(plan_with_line_offers(
        &form,
        &[source, sink],
        &placements,
        &[ConnectionBase::Local, ConnectionBase::WebSocket],
        1,
        conduit_signal::TRIGGER_ENCODED_LEN,
        &[],
    )
    .is_err());
}

/// EOF from stdin (Ok(0)) must produce a structured error, not a fabricated trigger.
#[test]
fn complete_trigger_wait_rejects_eof() {
    use conduit_kernel::scheduler::SchedulerStatus;
    let mut source = DistributedToggleSource::prepare().expect("prepare");
    // Step until the scheduler issues the first await-trigger request.
    let request = loop {
        if let Some(req) = source.scheduler.next_host_request() {
            break req;
        }
        match source.scheduler.step().expect("step") {
            SchedulerStatus::Progress { .. } => {}
            other => panic!("unexpected status: {other:?}"),
        }
    };
    // EOF reader always returns Ok(0).
    let mut eof_stdin: &[u8] = b"";
    let mut report = Vec::<u8>::new();
    let result = source.complete_trigger_wait(request, &mut report, &mut eof_stdin, 0);
    assert!(
        result.is_err(),
        "EOF must be a structured error, not a successful trigger"
    );
    let msg = result.unwrap_err();
    assert!(msg.contains("EOF"), "error should mention EOF, got: {msg}");
}

/// A read error must produce a structured error, not a fabricated trigger.
#[test]
fn complete_trigger_wait_rejects_read_error() {
    use conduit_kernel::scheduler::SchedulerStatus;
    use std::io;
    struct ErrorReader;
    impl io::Read for ErrorReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }
    }
    impl io::BufRead for ErrorReader {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }
        fn consume(&mut self, _amt: usize) {}
    }

    let mut source = DistributedToggleSource::prepare().expect("prepare");
    let request = loop {
        if let Some(req) = source.scheduler.next_host_request() {
            break req;
        }
        match source.scheduler.step().expect("step") {
            SchedulerStatus::Progress { .. } => {}
            other => panic!("unexpected status: {other:?}"),
        }
    };
    let mut report = Vec::<u8>::new();
    let result = source.complete_trigger_wait(request, &mut report, &mut ErrorReader, 0);
    assert!(
        result.is_err(),
        "read error must be a structured error, not a successful trigger"
    );
}
