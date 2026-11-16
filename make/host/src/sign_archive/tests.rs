use super::*;
const SOURCE: &str = r#"host archive-test {
 schema = 1
 target = {architecture: "thumbv6m", machine: "pico-w", board: "pico-w"}
 base = {kind: "kernel/sign-archive", implementation: "kernel/affine-prefix-sign-archive@1", initialization: {maximum_retained_events: 32768, maximum_canonical_bytes: 2097152, template_events: 128, template_runs: 16, remote_streams: 2, maximum_archive_backing_bytes: 8192, maximum_transfer_overlap_bytes: 12288}}
 limits = {static_memory_bytes: 229376, heap_arena_bytes: 110592, queue_items: 20, buffered_bytes: 24576, active_instances: 8, operation_slots: 16, timer_slots: 4, line_sessions: 2, evidence_items: 64}
}"#;
fn read(source: &str) -> Result<Option<DeclaredSignArchiveBounds>, String> {
    DeclaredSignArchiveBounds::from_configuration(
        &crate::parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?,
    )
}
#[test]
fn logical_archive_quota_is_separate_from_unchanged_active_and_physical_limits() {
    let config = crate::parse_host_configuration_conduit(SOURCE).unwrap();
    let limits = config.limits.clone();
    let b = DeclaredSignArchiveBounds::from_configuration(&config)
        .unwrap()
        .unwrap();
    assert_eq!(b.maximum_canonical_bytes, 2097152);
    assert_eq!(b.maximum_retained_events, 32768);
    assert_eq!(config.limits, limits);
    assert_eq!(limits.evidence_items, 64);
    assert_eq!(limits.buffered_bytes, 24576);
    let absent = SOURCE
        .lines()
        .filter(|l| !l.trim_start().starts_with("base ="))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(read(&absent).unwrap(), None);
    let smaller = SOURCE
        .replace("template_events: 128", "template_events: 1")
        .replace("template_runs: 16", "template_runs: 1")
        .replace("remote_streams: 2", "remote_streams: 0");
    assert!(
        read(&smaller).is_ok(),
        "no invented example-specific minima; local archives need no remote stream"
    );
}
#[test]
fn malformed_unknown_missing_and_overgrant_fields_refuse() {
    for changed in [
        SOURCE.replace(
            "maximum_retained_events: 32768",
            "maximum_retained_events: 0",
        ),
        SOURCE.replace(
            "maximum_canonical_bytes: 2097152",
            "maximum_canonical_bytes: 0",
        ),
        SOURCE.replace("template_runs: 16", "template_runs: 0"),
        SOURCE.replace("remote_streams: 2", "remote_streams: 65536"),
        SOURCE.replace(
            "maximum_archive_backing_bytes: 8192",
            "maximum_archive_backing_bytes: 110593",
        ),
        SOURCE.replace(
            "maximum_transfer_overlap_bytes: 12288",
            "maximum_transfer_overlap_bytes: 110593",
        ),
        SOURCE.replace("template_events: 128, ", ""),
        SOURCE.replace("remote_streams: 2", "other: 2"),
    ] {
        assert!(read(&changed).is_err());
    }
    let base = SOURCE
        .lines()
        .find(|l| l.trim_start().starts_with("base ="))
        .unwrap();
    assert!(read(&SOURCE.replace(base, &format!("{base}\n{base}"))).is_err());
    assert!(SignArchivePackage::for_targets(vec![]).is_err());
}
