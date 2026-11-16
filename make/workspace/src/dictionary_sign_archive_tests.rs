//! Explicit workspace availability only; no target archive is initialized.
use conduit_host_make::{check_host_configuration, parse_host_configuration_conduit};
const ARCHIVE: &str = r#"base = {kind: "kernel/sign-archive", implementation: "kernel/dictionary-prefix-sign-archive@1", initialization: {maximum_retained_events: 32768, maximum_canonical_bytes: 2097152, template_events: 128, template_count: 4, segment_runs: 48, request_lanes: 2, remote_streams: 2, maximum_archive_backing_bytes: 8192, maximum_transfer_overlap_bytes: 12288}}"#;
fn source(target: &str, archive: &str) -> String {
    format!(
        r#"host dictionary-test {{
 schema = 1
 target = {{{target}}}
 {archive}
 limits = {{static_memory_bytes: 229376, heap_arena_bytes: 110592, queue_items: 20, buffered_bytes: 24576, active_instances: 8, operation_slots: 16, timer_slots: 4, line_sessions: 2, evidence_items: 64}}
}}"#
    )
}
#[test]
fn only_two_explicit_hardware_targets_can_select_dictionary_construction() {
    let packages = super::package_set();
    let catalog = super::catalog();
    for target in [
        r#"architecture: "xtensa-lx6", machine: "hw-463-esp-wroom-32", board: "hw-463-esp-wroom-32", make_descriptor: "sha256:3015c678e54e8ae4dc1d0552a30ebdcb118b5c55dfa372252a8a5b86847dc255""#,
        r#"architecture: "thumbv6m", machine: "pico-w", board: "pico-w""#,
    ] {
        let selected = check_host_configuration(
            parse_host_configuration_conduit(&source(target, ARCHIVE)).unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        assert!(selected
            .resolved_bases()
            .iter()
            .any(|(kind, implementation)| kind == "kernel/sign-archive"
                && implementation == "kernel/dictionary-prefix-sign-archive@1"));
        let absent = check_host_configuration(
            parse_host_configuration_conduit(&source(target, "")).unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        assert!(!absent
            .resolved_bases()
            .iter()
            .any(|(kind, _)| kind == "kernel/sign-archive"));
        assert_eq!(
            selected.configuration().limits,
            absent.configuration().limits
        );
    }
    for target in [
        r#"architecture: "x86_64", machine: "computer", os: "linux""#,
        r#"architecture: "riscv32", machine: "esp32-c3", board: "esp32-c3""#,
    ] {
        assert!(check_host_configuration(
            parse_host_configuration_conduit(&source(target, ARCHIVE)).unwrap(),
            &catalog,
            &packages
        )
        .is_err());
    }
    let target = r#"architecture: "thumbv6m", machine: "pico-w", board: "pico-w""#;
    for invalid in [
        ARCHIVE.replace("dictionary-prefix", "affine-prefix"),
        ARCHIVE.replace("template_count: 4", "template_count: 0"),
    ] {
        assert!(check_host_configuration(
            parse_host_configuration_conduit(&source(target, &invalid)).unwrap(),
            &catalog,
            &packages
        )
        .is_err());
    }
}
