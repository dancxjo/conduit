use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

const TODO: &str = "plot todo/actions (\n >> add: Text... <=256B\n >> complete: Text... <=64B\n fragment: face/fragment@1... <=64KiB >>\n action conduit.intent/todo-add@1 text: Text >> add\n action conduit.intent/todo-complete@1 target: Text >> complete\n face >> fragment\n) {\n}\n";

fn check(source: &str) -> Result<conduit_plot::CheckedSyntaxDocument, String> {
    let parsed = parse_syntax_document(source);
    if let Some(error) = parsed.diagnostics.first() {
        return Err(error.message.clone());
    }
    check_syntax_document(&parsed, &StartupCatalog::new()).map_err(|error| error.message)
}

#[test]
fn todo_action_and_fragment_ports_are_checked_and_change_plot_identity() {
    let checked = check(TODO).unwrap();
    let plot = &checked.plots[0];
    assert_eq!(plot.action_bindings.len(), 2);
    assert_eq!(plot.action_bindings[0].input_port.as_str(), "add");
    assert_eq!(plot.action_bindings[0].value_kind.as_str(), "value/text");
    assert_eq!(plot.action_bindings[0].maximum_bytes, 256);
    assert_eq!(plot.face_fragment_output.as_deref(), Some("fragment"));
    let renamed =
        check(&TODO.replace("conduit.intent/todo-add@1", "conduit.intent/todo-create@1")).unwrap();
    assert_ne!(plot.checked_plot_id, renamed.plots[0].checked_plot_id);
}

#[test]
fn action_binding_refuses_missing_wrong_direction_type_and_ambiguous_input() {
    for (source, expected) in [
        (
            TODO.replace("text: Text >> add", "text: Text >> absent"),
            "input port is missing",
        ),
        (
            TODO.replace("text: Text >> add", "text: Boolean >> add"),
            "types differ",
        ),
        (
            TODO.replace("target: Text >> complete", "target: Text >> fragment"),
            "open input Flow",
        ),
        (
            TODO.replace("target: Text >> complete", "target: Text >> add"),
            "ambiguous Face action input",
        ),
        (
            TODO.replace(
                "conduit.intent/todo-complete@1",
                "conduit.intent/todo-add@1",
            ),
            "duplicate Face action intent",
        ),
        (
            TODO.replace("face >> fragment", "face >> absent"),
            "output port is missing",
        ),
        (
            TODO.replace("face >> fragment", "face >> add"),
            "open output Flow",
        ),
        (TODO.replace("face/fragment@1", "value/text"), "wrong type"),
        (TODO.replace("<=256B", "<=8KiB"), "finite byte bound"),
        (
            TODO.replace("face >> fragment\n", ""),
            "require one fragment output",
        ),
        (TODO.replace("<=64KiB", "<=128KiB"), "finite byte bound"),
    ] {
        let error = check(&source).unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[test]
fn duplicate_face_declaration_and_malformed_argument_refuse_at_parse() {
    for source in [
        TODO.replace("face >> fragment", "face >> fragment\n face >> fragment"),
        TODO.replace("text: Text >> add", "text: Text extra >> add"),
    ] {
        assert!(check(&source).is_err());
    }
}
