use crate::{
    parse_syntax_document, Argument, BackStatement, BinaryOperator, ConstructionRole, CordStage,
    CstTokenKind, Expression, ExpressionProjection, ExpressionSyntax, FormCompletionPolicy,
    RuntimePortDirection, RuntimePortTemporal, TypeDefinitionSyntax, TypeExpressionSyntax,
    TypeVariantPayloadSyntax,
};
use alloc::vec::Vec;

#[test]
fn local_forms_are_lossless_nested_source_syntax() {
    let source = "form outer {\n form helper (\n  >> value: Count\n  mapped: Count >>\n ) = (. + 1)\n child: helper\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.round_trip(), source);
    assert_eq!(document.forms.len(), 1);
    assert_eq!(document.forms[0].local_forms.len(), 1);
    assert_eq!(document.forms[0].local_forms[0].name.text, "helper");
    assert!(document.forms[0].local_forms[0].local_forms.is_empty());
}

#[test]
fn kind_parameter_retains_its_exact_named_fore_constraint() {
    let source = "form each (\n transform: kind (\n  >> value: Text\n  mapped: Text >>\n )\n >> values: Text...|\n mapped: Text...| >>\n) {\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let parameter = &document.forms[0].front.kind_parameters[0];
    assert_eq!(parameter.name.text, "transform");
    assert_eq!(parameter.front.runtime_ports.len(), 2);
    assert_eq!(parameter.front.runtime_ports[0].name.text, "value");
    assert_eq!(parameter.front.runtime_ports[1].name.text, "mapped");
    assert_eq!(document.round_trip(), source);
}

#[test]
fn checked_pattern_refinement_is_lossless_front_syntax() {
    let source = "form code (\n >> value: Text <= 16B ~ /[A-Z]{2}[0-9]{4}/\n) {\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.round_trip(), source);
    let crate::ValueRefinement::TextPattern { source, .. } =
        &document.forms[0].front.runtime_ports[0].refinements[0]
    else {
        panic!("expected the authored pattern refinement")
    };
    assert_eq!(source.text, "[A-Z]{2}[0-9]{4}");
}

#[test]
fn portable_lookahead_and_group_profiles_are_lossless_front_syntax() {
    let source = "form code (\n >> value: Text <= 16B ~ /(?=AB)(?:A)(?<tail>.)/\n) {\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.round_trip(), source);
    let crate::ValueRefinement::TextPattern { source, .. } =
        &document.forms[0].front.runtime_ports[0].refinements[0]
    else {
        panic!("expected the authored pattern refinement")
    };
    assert_eq!(source.text, "(?=AB)(?:A)(?<tail>.)");
}

#[test]
fn range_membership_and_composition_are_lossless_front_syntax() {
    let source = "form choice (\n >> value: Count in 1..=4 in [2, 3, 4]\n) {\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.round_trip(), source);
    let refinements = &document.forms[0].front.runtime_ports[0].refinements;
    assert_eq!(refinements.len(), 2);
    assert!(matches!(
        &refinements[0],
        crate::ValueRefinement::Range {
            minimum,
            maximum,
            minimum_endpoint: crate::RefinementIntervalEndpoint::Inclusive,
            maximum_endpoint: crate::RefinementIntervalEndpoint::Inclusive,
            ..
        } if minimum.as_ref().is_some_and(|value| value.text == "1")
            && maximum.as_ref().is_some_and(|value| value.text == "4")
    ));
    assert!(matches!(
        &refinements[1],
        crate::ValueRefinement::Membership { members, .. }
            if members.iter().map(|member| member.text.as_str()).collect::<Vec<_>>()
                == ["2", "3", "4"]
    ));
}

#[test]
fn open_ended_ranges_remain_lossless_until_the_type_supplies_their_bounds() {
    let source = "form bounded (\n >> low: Count in ..=4\n >> high: Scalar in 1.000000..\n) {\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.round_trip(), source);
    let low = &document.forms[0].front.runtime_ports[0].refinements[0];
    let high = &document.forms[0].front.runtime_ports[1].refinements[0];
    assert!(matches!(
        low,
        crate::ValueRefinement::Range {
            minimum: None,
            maximum: Some(maximum),
            ..
        } if maximum.text == "4"
    ));
    assert!(matches!(
        high,
        crate::ValueRefinement::Range {
            minimum: Some(minimum),
            maximum: None,
            ..
        } if minimum.text == "1.000000"
    ));
}

#[test]
fn forms_are_live_by_default_and_completion_is_explicit() {
    let source = "form live {\n    source: text/literal(\"ready\")\n}\nform finite {\n    source: text/literal(\"done\")\n}.\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.forms[0].completion, FormCompletionPolicy::Live);
    assert_eq!(
        document.forms[1].completion,
        FormCompletionPolicy::SemanticCompletion
    );
    assert_eq!(document.round_trip(), source);
}

#[test]
fn with_headers_and_glyph_opt_out_are_lossless_document_structure() {
    let source = "sans glyphs\nwith time/every as cadence\nwith math/geometry/{vector2, matrix2}\n\nform example {\n    tick: cadence(1s)\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert!(!document.standard_glyphs);
    assert_eq!(document.uses.len(), 3);
    assert_eq!(document.uses[0].path, "time/every");
    assert_eq!(document.uses[0].alias.text, "cadence");
    assert_eq!(document.uses[1].path, "math/geometry/vector2");
    assert_eq!(document.uses[1].alias.text, "vector2");
    assert_eq!(document.uses[2].path, "math/geometry/matrix2");
    assert_eq!(document.uses[2].alias.text, "matrix2");
    assert_eq!(document.round_trip(), source);
}

#[test]
fn punctuation_gear_names_are_lossless_but_core_tokens_remain_grammar() {
    let source = "with time/default as ^^\nform example {\n  @: current/sample\n  input >> ^^ >> @ >> output\n}\n";
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.round_trip(), source);
    assert_eq!(parsed.uses[0].alias.text, "^^");
    let BackStatement::NamedGear(named) = &parsed.forms[0].back[0] else {
        panic!("expected configured glyph Gear")
    };
    assert_eq!(named.name.text, "@");

    for reserved in [">>", ">", "?", "!", "~", "."] {
        let source = alloc::format!("with time/default as {reserved}\nform example {{\n}}\n");
        assert!(
            !parse_syntax_document(&source).diagnostics.is_empty(),
            "{reserved}"
        );
    }
}

#[test]
fn authored_imports_have_one_explicit_finite_document_bound() {
    let mut source = alloc::string::String::new();
    for index in 0..=crate::MAXIMUM_USE_DECLARATIONS {
        source.push_str(&alloc::format!("with test/kind-{index}\n"));
    }
    source.push_str("form example {\n}\n");
    let document = parse_syntax_document(&source);
    assert!(document.forms().is_err());
    assert!(document.diagnostics[0].message.contains("import bound"));
}

#[test]
fn native_semantic_types_are_lossless_finite_syntax_not_rust_shapes() {
    let source = "type Note = U8 in 0..=127\n\ntype Position = {\n    x: Distance\n    y: Distance\n}\n\ntype MusicEvent =\n    note {\n        velocity: U8 in 0..=127\n        pitches: sequence Note <= 16\n    }\n    | rest\n\nform perform (\n    >> event: MusicEvent\n) {\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.round_trip(), source);
    assert_eq!(document.types.len(), 3);
    assert_eq!(document.types[0].name.text, "Note");
    assert!(matches!(
        &document.types[0].definition,
        TypeDefinitionSyntax::Scalar(TypeExpressionSyntax::Reference {
            value_type,
            refinements,
            ..
        }) if value_type.text == "U8" && refinements.len() == 1
    ));
    assert!(matches!(
        &document.types[1].definition,
        TypeDefinitionSyntax::Record(fields)
            if fields.iter().map(|field| field.name.text.as_str()).collect::<Vec<_>>() == ["x", "y"]
    ));
    let TypeDefinitionSyntax::Variant(cases) = &document.types[2].definition else {
        panic!("expected closed variant syntax")
    };
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0].tag.text, "note");
    assert_eq!(cases[1].tag.text, "rest");
    assert!(matches!(
        &match &cases[0].payload {
            TypeVariantPayloadSyntax::Record(fields) => &fields[1].value_type,
            _ => panic!("expected record payload"),
        },
        TypeExpressionSyntax::Sequence {
            minimum_items: 0,
            maximum_items: 16,
            ..
        }
    ));
    assert_eq!(
        document.forms[0].front.runtime_ports[0].value_type.text,
        "MusicEvent"
    );
}

#[test]
fn native_variant_case_can_carry_a_type_directly() {
    let document = parse_syntax_document(
        "type Refusal =\n    unavailable\n\ntype Outcome =\n    completed\n    | refused Refusal\n",
    );
    assert!(document.diagnostics.is_empty());
    let TypeDefinitionSyntax::Variant(cases) = &document.types[1].definition else {
        panic!("expected variant syntax")
    };
    assert!(matches!(
        &cases[1].payload,
        TypeVariantPayloadSyntax::Type(TypeExpressionSyntax::Reference { value_type, .. })
            if value_type.text == "Refusal"
    ));
}

#[test]
fn native_sequence_type_requires_one_explicit_finite_cardinality_bound() {
    for source in [
        "type Notes = sequence Note\n",
        "type Notes = sequence Note <= 0\n",
        "type Notes = sequence Note <= 257\n",
    ] {
        let document = parse_syntax_document(source);
        assert!(!document.diagnostics.is_empty(), "accepted {source}");
    }
}

#[test]
fn native_sequence_can_state_exact_nonzero_cardinality_bounds() {
    let document = parse_syntax_document("type Notes = sequence U8 in 2..=4\n");
    assert!(document.diagnostics.is_empty());
    assert!(matches!(
        &document.types[0].definition,
        TypeDefinitionSyntax::Scalar(TypeExpressionSyntax::Sequence {
            minimum_items: 2,
            maximum_items: 4,
            ..
        })
    ));
    for source in [
        "type Notes = sequence U8 in 4..=2\n",
        "type Notes = sequence U8 in 1..=257\n",
    ] {
        assert!(!parse_syntax_document(source).diagnostics.is_empty());
    }
}

#[test]
fn native_fixed_collection_requires_one_exact_finite_length() {
    let document = parse_syntax_document("type Quartet = collection U16 = 4\n");
    assert!(document.diagnostics.is_empty());
    assert!(matches!(
        &document.types[0].definition,
        TypeDefinitionSyntax::Scalar(TypeExpressionSyntax::Collection { length: 4, .. })
    ));
    for source in [
        "type Quartet = collection U16\n",
        "type Quartet = collection U16 = 257\n",
    ] {
        let document = parse_syntax_document(source);
        assert!(!document.diagnostics.is_empty(), "accepted {source}");
    }
}

#[test]
fn legacy_use_keyword_is_not_a_compatibility_spelling() {
    let document =
        parse_syntax_document("use time/every as cadence\nform example {\n tick: cadence(1s)\n}\n");
    assert!(document.uses.is_empty());
    assert_eq!(document.diagnostics.len(), 1);
}

#[test]
fn legacy_without_glyphs_header_is_not_a_compatibility_spelling() {
    let document = parse_syntax_document("without glyphs\nform example {\n}\n");
    assert_eq!(document.diagnostics.len(), 1);
    assert!(document.forms.is_empty());
}

#[test]
fn freestanding_completion_full_stop_is_rejected() {
    let document = parse_syntax_document("form invalid {\n    .\n}\n");
    assert_eq!(document.diagnostics.len(), 1);
    assert!(document.diagnostics[0]
        .message
        .contains("belongs after the form body"));
}

#[test]
fn english_completion_keyword_is_not_retained_as_an_alias() {
    let source = "form invalid {\n    complete\n}\n";
    let document = parse_syntax_document(source);
    let diagnostic = document.diagnostics.first().unwrap();
    assert_eq!(
        &source[diagnostic.span.start..diagnostic.span.end],
        "complete"
    );
    assert!(diagnostic.message.contains("after the form body"));
}

#[test]
fn legacy_single_greater_than_is_not_retained_as_a_cord_alias() {
    for source in [
        "form invalid {\n    source > sink\n}\n",
        "form invalid (\n    input: Text > output: Text\n) {\n}\n",
    ] {
        let document = parse_syntax_document(source);
        assert!(document.forms().is_err());
        assert!(document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "CND-FRM-019"
                && diagnostic.message.contains("use '>>'")));
    }
}

#[test]
fn comparisons_do_not_become_legacy_cord_diagnostics() {
    let source = "form comparison {\n    threshold = 3 > 2\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
}

#[test]
fn all_canonical_temporal_modalities_and_finite_bounds_are_retained() {
    let source = "form temporal (\n    >> one: Text\n    >> current: $Text\n    >> flow: Text...\n    >> closing: Text...|\n    >> maybe: Text?\n    current-maybe: $Text? >>\n    bounded: Text <= 4KiB >>\n) {\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let ports = &document.forms[0].front.runtime_ports;
    assert_eq!(ports[0].temporal, RuntimePortTemporal::Value);
    assert_eq!(ports[1].temporal, RuntimePortTemporal::Current);
    assert_eq!(
        ports[2].temporal,
        RuntimePortTemporal::Flow { closes: false }
    );
    assert_eq!(
        ports[3].temporal,
        RuntimePortTemporal::Flow { closes: true }
    );
    assert_eq!(ports[4].temporal, RuntimePortTemporal::OptionalValue);
    assert_eq!(ports[5].temporal, RuntimePortTemporal::CurrentOptional);
    assert_eq!(ports[0].maximum_bytes, Some(256));
    assert_eq!(ports[6].maximum_bytes, Some(4 * 1024));
}

#[test]
fn data_references_and_exact_width_integer_types_are_canonical_fore_types() {
    let source = "form systems (\n    >> saved: &Text\n    >> image: &media/image@4\n    >> word: U32\n    signed: I128 >>\n) {\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let ports = &document.forms[0].front.runtime_ports;
    assert_eq!(ports[0].value_type.text, "&Text");
    assert_eq!(ports[1].value_type.text, "&media/image@4");
    assert_eq!(ports[2].value_type.text, "U32");
    assert_eq!(ports[3].value_type.text, "I128");
}

#[test]
fn recursive_or_empty_data_reference_types_are_rejected() {
    for value_type in ["&", "&&Text"] {
        let source = alloc::format!("form bad (\n    >> value: {value_type}\n) {{\n}}\n");
        let document = parse_syntax_document(&source);
        assert!(document.forms().is_err(), "{value_type}");
        assert_eq!(document.diagnostics[0].code, "CND-FRM-019");
    }
}

#[test]
fn pure_expression_precedence_and_ternary_are_structural_not_opaque_text() {
    let source = "form expressions {\n    result = a || b && c | d ^ e & f == g < h >>> 2 + 3 * 4 ? yes : no\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let BackStatement::LocalValue(local) = &document.forms[0].back[0] else {
        panic!(
            "expression is a local value: {:?}",
            document.forms[0].back[0]
        );
    };
    let ExpressionSyntax::Conditional { condition, .. } = &local.value.syntax else {
        panic!("lowest-precedence ternary is explicit");
    };
    assert!(matches!(
        condition.as_ref(),
        ExpressionSyntax::Binary {
            operator: BinaryOperator::BooleanOr,
            ..
        }
    ));

    fn contains_multiply(expression: &ExpressionSyntax) -> bool {
        match expression {
            ExpressionSyntax::Binary {
                operator: BinaryOperator::Multiply,
                ..
            } => true,
            ExpressionSyntax::Binary { left, right, .. } => {
                contains_multiply(left) || contains_multiply(right)
            }
            _ => false,
        }
    }
    assert!(contains_multiply(condition));
}

#[test]
fn input_tuple_record_projection_and_semantic_calls_have_distinct_syntax() {
    let source =
        "form expressions {\n    tuple = (.field, .0, { reading, scaled: math/sin(.) })\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let BackStatement::LocalValue(local) = &document.forms[0].back[0] else {
        panic!("expression is a local value");
    };
    let ExpressionSyntax::Tuple { values, .. } = &local.value.syntax else {
        panic!("parenthesized comma expression is a tuple");
    };
    assert!(matches!(
        &values[0],
        ExpressionSyntax::Projection {
            member: ExpressionProjection::Field(_),
            ..
        }
    ));
    assert!(matches!(
        &values[1],
        ExpressionSyntax::Projection {
            member: ExpressionProjection::TupleIndex(_),
            ..
        }
    ));
    let ExpressionSyntax::Record { fields, .. } = &values[2] else {
        panic!("third tuple element is a record");
    };
    assert!(fields[0].punned);
    assert!(!fields[1].punned);
    assert!(matches!(
        fields[1].value,
        ExpressionSyntax::SemanticCall { ref kind, .. } if kind.text == "math/sin"
    ));
}

#[test]
fn parenthesized_runtime_expression_is_one_cord_stage() {
    let source = "form classify (\n    >> reading: Temperature\n    label: Text >>\n) {\n    reading >> (. > 30°C ? \"hot\" : \"fine\") >> label\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let BackStatement::Cord(cord) = &document.forms[0].back[0] else {
        panic!("body contains one cord");
    };
    assert!(matches!(
        &cord.stages[1],
        CordStage::PureExpression(Expression {
            syntax: ExpressionSyntax::Conditional { .. },
            ..
        })
    ));
}

#[test]
fn percentage_quantity_suffix_is_not_a_remainder_without_a_right_operand() {
    let source = "form classify (\n    >> charge: Ratio\n    label: Text >>\n) {\n    charge >> (. > 75% ? \"full\" : \"charging\") >> label\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.round_trip(), source);
}

#[test]
fn when_terminal_quiescence_and_cancellation_stages_are_not_ordinary_references() {
    let source = "form controls {\n    temperature >> when(. > limit) >> alarm\n    items| >> finish\n    items! >> explain\n    items; >> resting\n    deadline >> work~\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let cords = document.forms[0]
        .back
        .iter()
        .map(|statement| match statement {
            BackStatement::Cord(cord) => cord,
            _ => panic!("control specimen contains only cords"),
        })
        .collect::<Vec<_>>();
    assert!(matches!(cords[0].stages[1], CordStage::When(_)));
    assert!(matches!(
        cords[1].stages[0],
        CordStage::TerminalProjection {
            terminal: crate::TerminalProjection::NormalClose,
            ..
        }
    ));
    assert!(matches!(
        cords[2].stages[0],
        CordStage::TerminalProjection {
            terminal: crate::TerminalProjection::Abnormal,
            ..
        }
    ));
    assert!(matches!(
        cords[3].stages[0],
        CordStage::TerminalProjection {
            terminal: crate::TerminalProjection::Quiescence,
            ..
        }
    ));
    assert!(matches!(cords[4].stages[1], CordStage::Cancellation { .. }));
}

#[test]
fn where_is_not_a_when_compatibility_alias() {
    let source = "form no-alias {\n    value >> where(. > 0) >> sink\n}\n";
    let document = parse_syntax_document(source);
    assert!(document.diagnostics.is_empty());
    let BackStatement::Cord(cord) = &document.forms[0].back[0] else {
        panic!("source is a cord");
    };
    assert!(matches!(cord.stages[1], CordStage::InlineGear(_)));
}

#[test]
fn keep_parses_initializers_bounds_and_every_canonical_lifetime() {
    let source = "form retained {\n    step: keep Text\n    play: keep Integer for this play\n    wake: keep Text <= 128B for this wake\n    boot: keep Bytes <= 2MiB for this boot\n    body: keep Text? <= 4KiB for this body\n    life: keep Count(0) for life\n}\n";
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let retained = document.forms[0]
        .back
        .iter()
        .map(|statement| match statement {
            BackStatement::NamedGear(gear) => gear.retained.as_ref().unwrap(),
            _ => panic!("expected KEEP-backed Gear"),
        })
        .collect::<Vec<_>>();
    assert_eq!(retained[0].duration, crate::RetainedDuration::Step);
    assert_eq!(retained[1].duration, crate::RetainedDuration::Play);
    assert_eq!(retained[2].duration, crate::RetainedDuration::Wake);
    assert_eq!(retained[3].duration, crate::RetainedDuration::Boot);
    assert_eq!(retained[4].duration, crate::RetainedDuration::Body);
    assert_eq!(retained[5].duration, crate::RetainedDuration::Body);
    assert_eq!(retained[2].maximum_bytes, Some(128));
    assert_eq!(retained[3].maximum_bytes, Some(2 * 1024 * 1024));
    assert!(retained[4].optional);
    assert_eq!(retained[5].initial.as_ref().unwrap().text, "0");
}

#[test]
fn inline_comments_are_lossless_trivia_across_surface_roles() {
    let source = "# before definitions\nform peer ( # front opens\n    label: Text = \"channel #7\" # startup\n    input: Text >> output: Text # runtime front\n) { # back opens\n    # inside back\n    local = \"value # retained\" # local value\n    gear: text/constant(value = \"gear # retained\") # named gear\n    pool peers: peer(size = 2) # bounded pool\n    input >> gear >> output # cord\n} # form closes\nhost workstation { # host opens\n    profile = \"host # one\" # host declaration\n} # host closes\nbody household { # body opens\n    member = \"body # one\" # body declaration\n} # body closes\n";
    let document = parse_syntax_document(source);
    assert_eq!(document.round_trip(), source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.forms.len(), 1);
    assert_eq!(document.forms[0].front.startup_parameters.len(), 1);
    assert_eq!(document.forms[0].front.runtime_ports.len(), 2);
    assert_eq!(document.forms[0].back.len(), 4);
    assert_eq!(document.constructions.len(), 2);
    assert_eq!(document.constructions[0].role, ConstructionRole::Host);
    assert_eq!(document.constructions[1].role, ConstructionRole::Body);
    assert_eq!(
        document.forms[0].front.startup_parameters[0]
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
        panic!("one host construction document is required");
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
fn body_wardrobe_directives_are_role_specific_lossless_syntax() {
    let source = "with masks/native-graphical as graphical\nwith masks/spoken as spoken\nbody roseau {\n  wear graphical else spoken\n  want graphical over spoken\n}\n";
    let document = parse_syntax_document(source);
    assert_eq!(document.round_trip(), source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let [body] = document.constructions().unwrap() else {
        panic!("one Body is required");
    };
    assert_eq!(body.directives.len(), 2);
    let crate::ConstructionDirectiveSyntax::BodyWear { mask, fallback, .. } = &body.directives[0]
    else {
        panic!("first directive should wear a Mask");
    };
    assert_eq!(mask.text, "graphical");
    assert_eq!(fallback.as_ref().unwrap().text, "spoken");
    let crate::ConstructionDirectiveSyntax::BodyWant { masks, .. } = &body.directives[1] else {
        panic!("second directive should order policy");
    };
    assert_eq!(
        masks
            .iter()
            .map(|mask| mask.text.as_str())
            .collect::<Vec<_>>(),
        vec!["graphical", "spoken"]
    );

    let host = parse_syntax_document("host bad {\n  wear graphical else spoken\n}\n");
    assert!(!host.diagnostics.is_empty());
}

#[test]
fn canonical_clock_form_round_trips_with_named_and_inline_gears() {
    let source = "# canonical source\nform clock-demo {\n    clock: time/every(1s)\n    clock >> presentation/tick\n}\n";
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
fn canonical_front_keeps_startup_values_runtime_ports_and_shorthand_distinct() {
    let source = "form badge (\n    title: Text\n    tone: Tone = calm\n    state: $Signal >> view: WebFragment\n) {\n    hero: web/hero(title, tone)\n    state >> hero.state\n    hero >> view\n}\n";
    let document = parse_syntax_document(source);
    let form = &document.forms().expect("front parses")[0];

    assert_eq!(document.round_trip(), source);
    assert_eq!(form.front.startup_parameters.len(), 2);
    assert_eq!(form.front.startup_parameters[0].name.text, "title");
    assert!(form.front.startup_parameters[0].default.is_none());
    assert_eq!(
        form.front.startup_parameters[1]
            .default
            .as_ref()
            .expect("tone has a default")
            .text,
        "calm"
    );
    assert_eq!(form.front.runtime_ports.len(), 2);
    assert_eq!(
        form.front.runtime_ports[0].direction,
        RuntimePortDirection::Input
    );
    assert_eq!(
        form.front.runtime_ports[1].direction,
        RuntimePortDirection::Output
    );
    let shorthand = form
        .front
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
fn named_type_parameters_are_not_runtime_startup_values() {
    let document = crate::parse_syntax_document(
        "form identity (\n item: type\n limit: Count = 1\n >> value: item\n result: item >>\n) {\n}\n",
    );
    let form = &document.forms().unwrap()[0];
    assert_eq!(form.front.type_parameters.len(), 1);
    assert_eq!(form.front.type_parameters[0].name.text, "item");
    assert_eq!(form.front.startup_parameters.len(), 1);
    assert_eq!(form.front.startup_parameters[0].name.text, "limit");
}

#[test]
fn canonical_duplex_front_has_auxiliary_ports_without_a_shorthand_path() {
    let source = include_str!("../../../forms/socket-client/main.conduit");
    let document = parse_syntax_document(source);
    let form = &document.forms().expect("duplex front parses")[0];

    assert_eq!(document.round_trip(), source);
    assert_eq!(form.front.startup_parameters.len(), 1);
    assert_eq!(form.front.runtime_ports.len(), 3);
    assert_eq!(form.front.runtime_ports[0].name.text, "send");
    assert_eq!(
        form.front.runtime_ports[0].temporal,
        RuntimePortTemporal::Flow { closes: true }
    );
    assert_eq!(
        form.front.runtime_ports[0].direction,
        RuntimePortDirection::Input
    );
    assert_eq!(form.front.runtime_ports[1].name.text, "recv");
    assert_eq!(
        form.front.runtime_ports[1].temporal,
        RuntimePortTemporal::Flow { closes: true }
    );
    assert_eq!(
        form.front.runtime_ports[1].direction,
        RuntimePortDirection::Output
    );
    assert_eq!(form.front.runtime_ports[2].value_type.text, "Boolean");
    assert_eq!(
        form.front.runtime_ports[2].temporal,
        RuntimePortTemporal::Current
    );
    assert!(form.front.shorthand.is_none());
}

#[test]
fn canonical_back_represents_values_named_arguments_and_anonymous_gears() {
    let source = "form demo {\n    freq = 1s\n    clock: time/every(freq = 1s)\n    time/every(freq) >> sensors/read\n}\n";
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
    let parameter = &form.front.startup_parameters[0];
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
            "form bad (\n    a: A >> b: B >> c: C\n) {\n}\n",
            "malformed front arrows",
        ),
        (
            "form bad (\n    a: A >> b: B\n    c: C >> d: D\n) {\n}\n",
            "more than one shorthand front pair",
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
            "form bad {\n    value = source >> sink\n}\n",
            "expression cannot appear as a graph stage",
        ),
        (
            "form bad {\n    value = first = second\n}\n",
            "value = first = second",
        ),
        (
            "form bad (title: Text\n) {\n}\n",
            "front declarations must follow",
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
        "form bad (\n    >> port: $Tick...\n) {\n}\n",
        "form bad (\n    >> port: $Tick...|\n) {\n}\n",
        "form bad (\n    >> port: $\n) {\n}\n",
        "form bad (\n    >> port: ...\n) {\n}\n",
    ] {
        let document = parse_syntax_document(source);
        assert_eq!(document.diagnostics[0].code, "CND-FRM-019", "{source}");
        assert_eq!(document.round_trip(), source);
    }
}

#[test]
fn quoted_text_is_a_distinct_lossless_graph_stage() {
    let source = "form hello {\n    \"Hello, world.\" >> text/upper\n}\n";
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
    let source = "form greet (\n    greeting: Text = \"Hello\"\n    name: Text >> text: Text\n) {\n    join: text/join(greeting)\n    name >> join >> text\n}\n\nform welcome {\n    hello: greet(\"Welcome\")\n}\n";
    let document = parse_syntax_document(source);
    let forms = document.forms().expect("both forms parse");

    assert_eq!(forms.len(), 2);
    assert_eq!(forms[0].name.text, "greet");
    assert_eq!(forms[1].name.text, "welcome");
    assert_eq!(document.round_trip(), source);
}

#[test]
fn expression_body_is_one_lossless_sugar_for_an_explicit_cord() {
    let source = "form increment (\n    >> value: U8\n    result: U8 >>\n) = (. + 1)\n";
    let document = parse_syntax_document(source);
    assert!(document.diagnostics.is_empty());
    let BackStatement::Cord(cord) = &document.forms[0].back[0] else {
        panic!("expression body lowers to one ordinary Cord")
    };
    assert!(matches!(
        cord.stages.as_slice(),
        [
            CordStage::Reference(input),
            CordStage::PureExpression(_),
            CordStage::Reference(output)
        ] if input.text == "value" && output.text == "result"
    ));

    let ambiguous = parse_syntax_document(
        "form ambiguous (\n    >> left: U8\n    >> right: U8\n    result: U8 >>\n) = (. + 1)\n",
    );
    assert_eq!(ambiguous.diagnostics.len(), 1);
    assert!(ambiguous.diagnostics[0]
        .message
        .contains("write an ordinary explicit Form back"));
}

#[test]
fn canonical_parser_handles_inline_form_calls_and_quoted_punctuation() {
    let source = "form demo {\n    label = \"{ready} > waiting\"\n    greet(\"hello\") >> presentation/text\n}\n";
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
