use conduit_panel::parse;
use conduit_runtime::{Registry, RunIo};

#[test]
fn display_text_uses_the_presentation_channel_not_process_stdout() {
    let panel = parse(
        r#"
            panel 1
            node message : std/literal { value = "visible text" }
            node display : display/text
            cord message.value -> display.text
        "#,
    )
    .expect("display panel parses");
    let registry = Registry::hosted_primitives();
    let resolved = registry.resolve(&panel).expect("display panel resolves");
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();

    resolved
        .run_batch(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        })
        .expect("display panel executes");

    assert!(output.is_empty());
    assert!(error.is_empty());
    assert_eq!(display, b"visible text");
}

#[test]
fn tee_node_duplicates_flow_to_multiple_sinks() {
    let panel = parse(include_str!("../../../examples/flow-tee.panel")).expect("tee panel parses");

    let registry = Registry::hosted_primitives();
    let resolved = registry.resolve(&panel).expect("tee panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();

    resolved
        .run_batch(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        })
        .expect("tee panel executes");

    assert_eq!(output, b"one value, two coupled branches");
    assert_eq!(error, b"one value, two coupled branches");
}

#[test]
fn fallback_node_selects_primary_or_fallback() {
    let panel = parse(
        r#"
            panel 1
            node primary : std/literal { value = "primary data" }
            node secondary : std/literal { value = "fallback data" }
            node router : flow/fallback
            node encoded : text/encode-utf8
            node sink : io/stdout
            cord primary.value -> router.primary
            cord secondary.value -> router.fallback
            cord router.selected -> encoded.text
            cord encoded.bytes -> sink.bytes
        "#,
    )
    .expect("fallback panel parses");

    let registry = Registry::hosted_primitives();
    let resolved = registry.resolve(&panel).expect("fallback panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();

    resolved
        .run_batch(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        })
        .expect("fallback panel executes");

    assert_eq!(output, b"primary data");
}

#[test]
fn compatibility_batch_projects_the_first_merge_value() {
    let panel =
        parse(include_str!("../../../examples/flow-merge.panel")).expect("merge panel parses");

    let registry = Registry::hosted_primitives();
    let resolved = registry.resolve(&panel).expect("merge panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();

    resolved
        .run_batch(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        })
        .expect("merge panel executes");

    assert_eq!(output, b"first");
}
