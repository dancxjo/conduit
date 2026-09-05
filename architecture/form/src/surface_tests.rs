use crate::{
    parse_syntax_document, Argument, BackStatement, ConstructionRole, CordStage, CstTokenKind,
    ExpressionSyntax, RuntimePortDirection, RuntimePortTemporal,
};
use alloc::vec::Vec;

#[test]
fn inline_comments_are_lossless_trivia_across_surface_roles() {
    let source = "# before definitions\nform peer ( # face opens\n    label: Text = \"channel #7\" # startup\n    input: Text > output: Text # runtime face\n) { # back opens\n    # inside back\n    local = \"value # retained\" # local value\n    gear: text/constant(value = \"gear # retained\") # named gear\n    pool peers: peer(size = 2) # bounded pool\n    input > gear > output # cord\n} # form closes\nhost workstation { # host opens\n    profile = \"host # one\" # host declaration\n} # host closes\nbody household { # body opens\n    member = \"body # one\" # body declaration\n} # body closes\n";
    let document = parse_syntax_document(source);
    assert_eq!(document.round_trip(), source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.forms.len(), 1);
    assert_eq!(document.forms[0].face.startup_parameters.len(), 1);
    assert_eq!(document.forms[0].face.runtime_ports.len(), 2);
    assert_eq!(document.forms[0].back.len(), 4);
    assert_eq!(document.constructions.len(), 2);
    assert_eq!(document.constructions[0].role, ConstructionRole::Host);
    assert_eq!(document.constructions[1].role, ConstructionRole::Body);
    assert_eq!(
        document.forms[0].face.startup_parameters[0]
            .default
            .as_ref()
            .unwrap()
            .text,
        "\"channel #7\""
    );
    assert_eq!(
        document.constructions[0].declarations[0].value.text,
        "\"host # one\""
    );

    let comments = document
        .tokens
        .iter()
        .filter(|token| token.kind == CstTokenKind::Comment)
        .map(|token| token.text.as_str())
        .collect::<Vec<_>>();
    assert!(comments.contains(&"# startup"));
    assert!(comments.contains(&"# cord"));
    assert!(!comments.iter().any(|comment| comment.contains("retained")));
}

#[test]
fn malformed_statement_before_inline_comment_keeps_its_exact_span() {
    let source = "form bad {\n    clock: # missing kind\n}\n";
    let document = parse_syntax_document(source);
    let diagnostic = document.diagnostics.first().unwrap();
    assert_eq!(
        &source[diagnostic.span.start..diagnostic.span.end],
        "clock:"
    );
    assert_eq!(diagnostic.span.line, 2);
    assert_eq!(diagnostic.span.column, 5);
    assert_eq!(document.round_trip(), source);
}

#[test]
fn canonical_document_roles_share_tokens_declarations_values_and_diagnostics() {
    let source = "host specimen {\n  schema = 1\n  target = {architecture: \"x86_64\", machine: \"workstation\"}\n  base = {kind: \"clock/monotonic\", implementations: [\"hosted/monotonic-clock@1\"]}\n}\n";
    let document = parse_syntax_document(source);
    assert_eq!(document.round_trip(), source);
    assert!(!document.tokens.is_empty());
    assert!(document.forms.is_empty());
    let [host] = document.constructions().expect("Host role parses") else {
        panic!("one Host construction document is required");
    };
    assert_eq!(host.role, ConstructionRole::Host);
    assert_eq!(host.name.text, "specimen");
    assert_eq!(host.declarations.len(), 3);
    assert!(matches!(
        host.declarations[1].value.syntax,
        ExpressionSyntax::Record { .. }
    ));
    assert!(matches!(
        host.declarations[2].value.syntax,
        ExpressionSyntax::Record { .. }
    ));

    let malformed = parse_syntax_document(
        "body specimen {\n  host = {name: \"one\", spore: {join_mode: }}\n}\n",
    );
    let diagnostic = malformed
        .diagnostics
        .first()
        .expect("malformed structured value uses the canonical diagnostic path");
    assert_eq!(diagnostic.code, "CND-FRM-019");
    assert_eq!(diagnostic.span.line, 2);
}

#[test]
fn canonical_clock_form_round_trips_with_named_and_inline_gears() {
    let source = "# canonical source\nform clock-demo {\n    clock: time/every(1s)\n    clock > presentation/tick\n}\n";
    let document = parse_syntax_document(source);
    let forms = document.forms().expect("canonical form parses");

    assert_eq!(document.round_trip(), source);
    assert_eq!(forms.len(), 1);
    assert_eq!(forms[0].name.text, "clock-demo");
    let BackStatement::NamedGear(clock) = &forms[0].back[0] else {
        panic!("first statement should be a named gear");
    };
    assert_eq!(clock.name.text, "clock");
    assert_eq!(clock.invocation.kind.text, "time/every");
    assert!(matches!(
        &clock.invocation.arguments[..],
        [Argument::Positional(expression)] if expression.text == "1s"
    ));
    let BackStatement::Cord(cord) = &forms[0].back[1] else {
        panic!("second statement should be a cord");
    };
    assert!(matches!(
        &cord.stages[..],
        [CordStage::Reference(clock), CordStage::InlineGear(tick)]
            if clock.text == "clock" && tick.kind.text == "presentation/tick"
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
    let BackStatement::NamedGear(hero) = &form.back[0] else {
        panic!("hero is a named gear");
    };
    assert_eq!(hero.invocation.arguments.len(), 2);
}

#[test]
fn canonical_duplex_face_has_auxiliary_ports_without_a_shorthand_path() {
    let source = include_str!("../../../forms/socket-client/main.conduit");
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
fn canonical_back_represents_values_named_arguments_and_anonymous_gears() {
    let source = "form demo {\n    freq = 1s\n    clock: time/every(freq = 1s)\n    time/every(freq) > sensors/read\n}\n";
    let document = parse_syntax_document(source);
    let form = &document.forms().expect("back parses")[0];

    let BackStatement::LocalValue(freq) = &form.back[0] else {
        panic!("freq is a local value");
    };
    assert_eq!(freq.name.text, "freq");
    assert_eq!(freq.value.text, "1s");
    let BackStatement::NamedGear(clock) = &form.back[1] else {
        panic!("clock is a named gear");
    };
    assert!(matches!(
        &clock.invocation.arguments[..],
        [Argument::Named { name, value, .. }]
            if name.text == "freq" && value.text == "1s"
    ));
    let BackStatement::Cord(cord) = &form.back[2] else {
        panic!("anonymous gears form a cord");
    };
    assert!(matches!(
        &cord.stages[..],
        [CordStage::InlineGear(_), CordStage::InlineGear(_)]
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
        ("form bad {\n    clock:\n}\n", "missing Gear Kind"),
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
        CordStage::InlineGear(call) if call.kind.text == "greet"
    ));
}

#[test]
fn canonical_parser_rejects_unbalanced_invocation_expressions() {
    let source = "form bad {\n    gear: time/every(nested(value)\n}\n";
    let document = parse_syntax_document(source);
    let diagnostic = document.diagnostics.first().expect("unbalanced call fails");

    assert_eq!(diagnostic.code, "CND-FRM-019");
    assert_eq!(document.round_trip(), source);
}
