use super::*;
use crate::parse_host_configuration_conduit;
const SOURCE: &str = r#"host declared-carrier {
 schema = 1
 target = {architecture: "thumbv6m", machine: "pico-w", board: "pico-w"}
 preparation = {authorizer: "seed-owner", maximum_assignment_bytes: 1024, maximum_runtime_state_bytes: 1024}
 execution = {authorizer: "seed-owner", maximum_active_worksets: 1}
 base = {kind: "line/fore-carrier", implementation: "fixture/unregistered", initialization: {authorizer: "seed-owner", maximum_lines: 2, maximum_frame_bytes: 512, maximum_payload_bytes: 460, maximum_buffered_bytes: 1024, maximum_in_flight_items: 1, maximum_frames: 100}}
 limits = {static_memory_bytes: 65536, heap_arena_bytes: 65536, queue_items: 64, buffered_bytes: 4096, active_instances: 8, operation_slots: 8, timer_slots: 8, line_sessions: 2, evidence_items: 64}
}"#;
fn read(source: &str) -> Result<Option<DeclaredForeCarrierBounds>, String> {
    DeclaredForeCarrierBounds::from_configuration(
        &parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?,
    )
}
#[test]
fn declaration_is_readable_without_resolving_or_initializing_a_board() {
    let bounds = read(SOURCE).unwrap().unwrap();
    assert_eq!(bounds.maximum_lines, 2);
    assert_eq!(bounds.maximum_payload_bytes, 460);
    // The deliberately unregistered implementation is merely declared. This
    // path cannot certify a target build, native possession or a Ready Line.
    let configuration = parse_host_configuration_conduit(SOURCE).unwrap();
    assert_eq!(
        configuration.bases[0].implementation.as_deref(),
        Some("fixture/unregistered")
    );
    assert_eq!(bounds.maximum_frames, 100);
    let absent = SOURCE
        .lines()
        .filter(|line| !line.trim_start().starts_with("base ="))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(read(&absent).unwrap(), None);
}
#[test]
fn declared_frame_backing_and_original_host_caps_still_refuse() {
    for changed in [
        SOURCE.replace("maximum_frame_bytes: 512", "maximum_frame_bytes: 511"),
        SOURCE.replace("line_sessions: 2", "line_sessions: 1"),
        SOURCE.replace("buffered_bytes: 4096", "buffered_bytes: 512"),
        SOURCE.replace("heap_arena_bytes: 65536", "heap_arena_bytes: 512"),
        SOURCE.replace("maximum_in_flight_items: 1", "maximum_in_flight_items: 2"),
        SOURCE.replace("maximum_frames: 100", "maximum_frames: 0"),
    ] {
        assert!(read(&changed).is_err());
    }
}
#[test]
fn declared_carrier_requires_unique_base_and_both_explicit_policies() {
    for prefix in ["preparation =", "execution ="] {
        let missing = SOURCE
            .lines()
            .filter(|line| !line.trim_start().starts_with(prefix))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(read(&missing).is_err());
    }
    let base = SOURCE
        .lines()
        .find(|line| line.trim_start().starts_with("base ="))
        .unwrap();
    assert!(read(&SOURCE.replace(base, &format!("{base}\n{base}"))).is_err());
}
