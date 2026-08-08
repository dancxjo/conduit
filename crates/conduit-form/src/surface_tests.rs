use crate::{
    parse_syntax_document, Argument, BackStatement, CordStage, RuntimePortDirection,
    RuntimePortTemporal,
};

#[test]
fn canonical_clock_form_round_trips_with_named_and_inline_cells() {
    let source = "# canonical source\nform clock-demo {\n    clock: time/every(1s)\n    clock > presentation/tick\n}\n";
    let document = parse_syntax_document(source);
    let forms = document.forms().expect("canonical form parses");

    assert_eq!(document.round_trip(), source);
    assert_eq!(forms.len(), 1);
    assert_eq!(forms[0].name.text, "clock-demo");
    let BackStatement::NamedCell(clock) = &forms[0].back[0] else {
        panic!("first statement should be a named cell");
    };
    assert_eq!(clock.name.text, "clock");
    assert_eq!(clock.invocation.operation.text, "time/every");
    assert!(matches!(
        &clock.invocation.arguments[..],
        [Argument::Positional(expression)] if expression.text == "1s"
    ));
    let BackStatement::Cord(cord) = &forms[0].back[1] else {
        panic!("second statement should be a cord");
    };
    assert!(matches!(
        &cord.stages[..],
        [CordStage::Reference(clock), CordStage::InlineCell(tick)]
            if clock.text == "clock" && tick.operation.text == "presentation/tick"
    ));
}

#[test]
fn canonical_face_keeps_startup_values_runtime_ports_and_shorthand_distinct() {
    let source = "form badge (\n    title: Text\n    tone: Tone = calm\n    state: $Signal > view: WebFragment\n) {\n    hero: web/hero(title, tone)\n    state > hero.state\n    hero > view\n}\n";
    let document = parse_syntax_document(source);
    let form = &document.forms().expect("face parses")[0];

    assert_eq!(document.round_trip(), source);
    assert_eq!(form.face.startup_parameters.len(), 2);
    assert_eq!(form.face.startup_parameters[0].name.text, "title");
    assert!(form.face.startup_parameters[0].default.is_none());
    assert_eq!(
        form.face.startup_parameters[1]
            .default
            .as_ref()
            .expect("tone has a default")
            .text,
        "calm"
    );
    assert_eq!(form.face.runtime_ports.len(), 2);
    assert_eq!(
        form.face.runtime_ports[0].direction,
        RuntimePortDirection::Input
    );
    assert_eq!(
        form.face.runtime_ports[1].direction,
        RuntimePortDirection::Output
    );
    let shorthand = form
        .face
        .shorthand
        .as_ref()
        .expect("central pair is recorded");
    assert_eq!(shorthand.input_port.text, "state");
    assert_eq!(shorthand.output_port.text, "view");
    let BackStatement::NamedCell(hero) = &form.back[0] else {
        panic!("hero is a named cell");
    };
    assert_eq!(hero.invocation.arguments.len(), 2);
}

#[test]
fn canonical_duplex_face_has_auxiliary_ports_without_a_shorthand_path() {
    let source = include_str!("../../../examples/socket-client.conduit");
    let document = parse_syntax_document(source);
    let form = &document.forms().expect("duplex face parses")[0];

    assert_eq!(document.round_trip(), source);
    assert_eq!(form.face.startup_parameters.len(), 1);
    assert_eq!(form.face.runtime_ports.len(), 3);
    assert_eq!(form.face.runtime_ports[0].name.text, "send");
    assert_eq!(
        form.face.runtime_ports[0].temporal,
        RuntimePortTemporal::Flow { closes: true }
    );
    assert_eq!(
        form.face.runtime_ports[0].direction,
        RuntimePortDirection::Input
    );
    assert_eq!(form.face.runtime_ports[1].name.text, "recv");
    assert_eq!(
        form.face.runtime_ports[1].temporal,
        RuntimePortTemporal::Flow { closes: true }
    );
    assert_eq!(
        form.face.runtime_ports[1].direction,
        RuntimePortDirection::Output
    );
    assert_eq!(form.face.runtime_ports[2].value_type.text, "Boolean");
    assert_eq!(
        form.face.runtime_ports[2].temporal,
        RuntimePortTemporal::Current
    );
    assert!(form.face.shorthand.is_none());
}

#[test]
fn canonical_back_represents_values_named_arguments_and_anonymous_cells() {
    let source = "form demo {\n    freq = 1s\n    clock: time/every(freq = 1s)\n    time/every(freq) > sensors/read\n}\n";
    let document = parse_syntax_document(source);
    let form = &document.forms().expect("back parses")[0];

    let BackStatement::LocalValue(freq) = &form.back[0] else {
        panic!("freq is a local value");
    };
    assert_eq!(freq.name.text, "freq");
    assert_eq!(freq.value.text, "1s");
    let BackStatement::NamedCell(clock) = &form.back[1] else {
        panic!("clock is a named cell");
    };
    assert!(matches!(
        &clock.invocation.arguments[..],
        [Argument::Named { name, value, .. }]
            if name.text == "freq" && value.text == "1s"
    ));
    let BackStatement::Cord(cord) = &form.back[2] else {
        panic!("anonymous cells form a cord");
    };
    assert!(matches!(
        &cord.stages[..],
        [CordStage::InlineCell(_), CordStage::InlineCell(_)]
    ));
}

#[test]
fn canonical_ast_spans_are_exact_utf8_byte_slices() {
    let source = "form café (\n    title: Text = \"héllo\"\n) {\n    card: web/hero(title)\n}\n";
    let document = parse_syntax_document(source);
    let form = &document.forms().expect("utf-8 form parses")[0];
    let parameter = &form.face.startup_parameters[0];
    let default = parameter.default.as_ref().expect("default exists");

    assert_eq!(&source[form.name.span.start..form.name.span.end], "café");
    assert_eq!(
        &source[parameter.span.start..parameter.span.end],
        "title: Text = \"héllo\""
    );
    assert_eq!(&source[default.span.start..default.span.end], "\"héllo\"");
    assert_eq!(default.span.line, 2);
    assert_eq!(default.span.column, 19);
}

#[test]
fn canonical_negative_corpus_has_stable_diagnostics_and_exact_spans() {
    let cases = [
        (
            "form bad (\n    a: A >> b: B\n) {\n}\n",
            "malformed face arrows",
        ),
        (
            "form bad (\n    a: A > b: B\n    c: C > d: D\n) {\n}\n",
            "more than one shorthand face pair",
        ),
        ("form bad {\n    clock:\n}\n", "missing cell operation"),
        (
            "form bad {\n    clock: time/every(freq = 1s, 2s)\n}\n",
            "positional argument cannot follow",
        ),
        (
            "form bad {\n    clock: time/every(1s) { nope }\n}\n",
            "cannot have a form back",
        ),
        (
            "form bad {\n    value = source > sink\n}\n",
            "expression cannot appear as a graph stage",
        ),
        (
            "form bad {\n    value = first = second\n}\n",
            "value = first = second",
        ),
        (
            "form bad (title: Text\n) {\n}\n",
            "face declarations must follow",
        ),
    ];

    for (source, message) in cases {
        let document = parse_syntax_document(source);
        let diagnostic = document.diagnostics.first().expect("source is rejected");
        assert_eq!(diagnostic.code, "CND-FRM-019", "{source}");
        assert!(
            diagnostic.message.contains(message),
            "{}: {}",
            diagnostic.message,
            source
        );
        assert!(!&source[diagnostic.span.start..diagnostic.span.end].is_empty());
        assert_eq!(document.round_trip(), source);
        assert!(document.forms.is_empty());
    }
}

#[test]
fn temporal_markers_cannot_be_combined_or_left_without_a_value_type() {
    for source in [
        "form bad (\n    > port: $Tick...\n) {\n}\n",
        "form bad (\n    > port: $Tick...|\n) {\n}\n",
        "form bad (\n    > port: $\n) {\n}\n",
        "form bad (\n    > port: ...\n) {\n}\n",
    ] {
        let document = parse_syntax_document(source);
        assert_eq!(document.diagnostics[0].code, "CND-FRM-019", "{source}");
        assert_eq!(document.round_trip(), source);
    }
}

#[test]
fn quoted_text_is_a_distinct_lossless_graph_stage() {
    let source = "form hello {\n    \"Hello, world.\" > text/upper\n}\n";
    let document = parse_syntax_document(source);
    let forms = document.forms().expect("quoted text is valid graph syntax");
    let BackStatement::Cord(cord) = &forms[0].back[0] else {
        panic!("back statement is a cord");
    };
    let CordStage::Literal(literal) = &cord.stages[0] else {
        panic!("first stage retains literal identity");
    };
    assert_eq!(
        &source[literal.span.start..literal.span.end],
        "\"Hello, world.\""
    );
    assert_eq!(document.round_trip(), source);
}

#[test]
fn canonical_parser_accepts_multiple_forms_without_semantic_lowering() {
    let source = "form greet (\n    greeting: Text = \"Hello\"\n    name: Text > text: Text\n) {\n    join: text/join(greeting)\n    name > join > text\n}\n\nform welcome {\n    hello: greet(\"Welcome\")\n}\n";
    let document = parse_syntax_document(source);
    let forms = document.forms().expect("both forms parse");

    assert_eq!(forms.len(), 2);
    assert_eq!(forms[0].name.text, "greet");
    assert_eq!(forms[1].name.text, "welcome");
    assert_eq!(document.round_trip(), source);
}

#[test]
fn canonical_parser_handles_inline_form_calls_and_quoted_punctuation() {
    let source = "form demo {\n    label = \"{ready} > waiting\"\n    greet(\"hello\") > presentation/text\n}\n";
    let document = parse_syntax_document(source);
    let form = &document
        .forms()
        .expect("quoted punctuation remains expression text")[0];

    let BackStatement::LocalValue(label) = &form.back[0] else {
        panic!("label is a local value");
    };
    assert_eq!(label.value.text, "\"{ready} > waiting\"");
    let BackStatement::Cord(cord) = &form.back[1] else {
        panic!("inline form call is a cord stage");
    };
    assert!(matches!(
        &cord.stages[0],
        CordStage::InlineCell(call) if call.operation.text == "greet"
    ));
}

#[test]
fn canonical_parser_rejects_unbalanced_invocation_expressions() {
    let source = "form bad {\n    cell: time/every(nested(value)\n}\n";
    let document = parse_syntax_document(source);
    let diagnostic = document.diagnostics.first().expect("unbalanced call fails");

    assert_eq!(diagnostic.code, "CND-FRM-019");
    assert_eq!(document.round_trip(), source);
}
