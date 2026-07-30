use conduit_panel::parse;
use conduit_runtime::{Registry, RunIo};

#[test]
fn probe_and_log_nodes_pass_through_flow_unmodified() {
    let panel = parse(
        r#"
            panel 1
            node source : conduit/literal { value = "telemetry item" }
            node probe : conduit/probe
            node log : conduit/log
            node sink : conduit/stdout
            cord source.out -> probe.in
            cord probe.out -> log.in
            cord log.out -> sink.in
        "#,
    )
    .expect("telemetry panel parses");

    let registry = Registry::default();
    let resolved = registry.resolve(&panel).expect("telemetry panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("telemetry panel runs");

    assert_eq!(output, b"telemetry item");
}

#[test]
fn record_and_assert_nodes_preserve_flow_semantics() {
    let panel = parse(
        r#"
            panel 1
            node source : conduit/literal { value = "tested payload" }
            node recorder : conduit/record
            node assertion : conduit/assert
            node sink : conduit/stdout
            cord source.out -> recorder.in
            cord recorder.out -> assertion.in
            cord assertion.out -> sink.in
        "#,
    )
    .expect("record & assert panel parses");

    let registry = Registry::default();
    let resolved = registry
        .resolve(&panel)
        .expect("record & assert panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("record & assert panel runs");

    assert_eq!(output, b"tested payload");
}
