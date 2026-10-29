//! Actual canonical emitter; transport/Root admission are outside this proof.
extern crate alloc;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "../src/streamed_identity.rs"]
mod actual;
use conduit_core::{ConnectionTrack, PortTemporal};
use conduit_plot::CheckedConnectionRef;
use sha2::{Digest, Sha256};
fn cord<'a>(source: &'a str, sink: &'a str) -> CheckedConnectionRef<'a> {
    CheckedConnectionRef {
        source_gear_id: source,
        source_port_id: "",
        sink_gear_id: sink,
        sink_port_id: "in:put",
        value_kind: "環境",
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
    }
}
fn legacy(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
#[test]
fn streamed_ids_preserve_legacy_utf8_empty_and_colon_boundary_semantics() {
    for (plot, source, sink) in [
        ("", "", ""),
        ("温度", "α:β", "γ"),
        ("a:b", "c", "d"),
        ("a", "b:c", "d"),
    ] {
        let c = cord(source, sink);
        let expected = legacy(&format!(
            "connection:{plot}:{}:{}:{}:{}:{}:{}:{}",
            c.source_gear_id,
            c.source_port_id,
            c.sink_gear_id,
            c.sink_port_id,
            c.value_kind,
            c.track.as_str(),
            c.temporal.as_str()
        ));
        assert_eq!(actual::connection(plot, c).unwrap().as_str(), expected);
        assert_eq!(
            actual::placement(plot, source, sink, "").unwrap().as_str(),
            legacy(&format!("placement:{plot}:{source}:{sink}:"))
        );
    }
    // The legacy delimiter transcript is intentionally unchanged, including its
    // boundary ambiguity; this repair must not silently revise identity rules.
    assert_eq!(
        actual::connection("a:b", cord("c", "d")).unwrap(),
        actual::connection("a", cord("b:c", "d")).unwrap()
    );
}
#[test]
fn long_transcript_allocates_only_the_exact_final_hex_owner() {
    let long = "temperature:環境".repeat(40);
    let (id, memory) =
        allocation_probe::observe(|| actual::connection(&long, cord(&long, &long)).unwrap());
    assert_eq!(id.as_str().len(), 64);
    assert_eq!(
        (
            memory.allocations,
            memory.reallocations,
            memory.requested_bytes
        ),
        (1, 0, 64)
    );
}
#[test]
fn output_and_collection_allocation_refusal_retire_partial_ids_and_retry_exactly() {
    let cords = [cord("first", "second"), cord("second", "third")];
    let expected = actual::connections("plot", &cords).unwrap();
    for bytes in [64, 2 * core::mem::size_of::<conduit_core::ConnectionId>()] {
        let (result, memory) = allocation_probe::observe(|| {
            let _refusal = allocation_probe::refuse_next_allocation_size(bytes);
            actual::connections("plot", &cords)
        });
        assert!(result.is_err());
        assert_eq!(memory.retained_bytes(), 0);
        assert_eq!(actual::connections("plot", &cords).unwrap(), expected);
    }
    let _refusal = allocation_probe::refuse_next_allocation_size(64);
    assert!(actual::placement("plot", "gear", "host", "capability").is_err());
}

#[path = "../src/contract/line_mechanism.rs"]
mod line_mechanism;
#[test]
fn line_mechanism_permissions_borrow_ids_without_allocating_comparison_strings() {
    let local = [conduit_core::BaseImplementationId::from(
        conduit_core::LOCAL_BASE_IMPLEMENTATION_ID,
    )];
    let remote = [conduit_core::BaseImplementationId::from("remote/line")];
    for (allowed, expected) in [
        (local.as_slice(), true),
        (remote.as_slice(), false),
        (&[], false),
    ] {
        let (actual, memory) = allocation_probe::observe(|| {
            let _refusal = allocation_probe::refuse_next_allocation_size(20);
            let policy = line_mechanism::LineMechanismPolicy::new(allowed);
            (
                policy.permits_local(),
                policy.permits_remote(&remote[0]),
                policy.permits_remote(&local[0]),
            )
        });
        assert_eq!(actual, (expected, !allowed.is_empty() && !expected, false));
        assert_eq!(memory.allocations + memory.reallocations, 0);
    }
}
