use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

struct Fixture {
    types: Vec<(String, StructuredInfoType)>,
    initialize: PreparedPortableExpressionEvaluator,
    steps: Vec<PreparedPortableExpressionEvaluator>,
    arc: PreparedPortableExpressionEvaluator,
    native_state: conduit_plot::CheckedNativeType,
}
impl Fixture {
    fn new() -> Self {
        let source = format!(
            "{}\n{}\n{}",
            include_str!("../identity.conduit"),
            include_str!("../types.conduit"),
            include_str!("../parser.conduit")
        );
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let prepare = |name| {
            let expanded =
                expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
                    .unwrap_or_else(|error| panic!("{name}: {error:?}"))
                    .expanded;
            let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value
            else {
                panic!("expression program")
            };

            PreparedPortableExpressionEvaluator::new(
                &PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
            )
            .unwrap()
        };
        Self {
            native_state: checked
                .native_types
                .iter()
                .find(|ty| ty.name == "LanguageParserState")
                .unwrap()
                .clone(),
            initialize: prepare("language-parser-initialize"),
            arc: prepare("language-parser-arc"),
            steps: [
                "language-parser-context",
                "language-parser-admit-0",
                "language-parser-admit-1",
                "language-parser-admit-2",
                "language-parser-admit-3",
                "language-parser-admit-4",
                "language-parser-admit-5",
                "language-parser-admit-6",
                "language-parser-admit-7",
                "language-parser-admit-8",
                "language-parser-admit-9",
                "language-parser-legal-shift",
                "language-parser-legal-reduce",
                "language-parser-legal-left",
                "language-parser-legal-right",
                "language-parser-legal",
                "language-parser-follow",
                "language-parser-follow",
                "language-parser-follow",
                "language-parser-follow",
                "language-parser-follow",
                "language-parser-apply",
            ]
            .into_iter()
            .map(prepare)
            .collect(),
            types: checked
                .native_types
                .iter()
                .map(|ty| (ty.name.clone(), ty.value_type.clone()))
                .collect(),
        }
    }
    fn native_ok(&self, state: &StructuredInfoValue) -> bool {
        let StructuredInfoValueShape::Record(fields) = state.shape() else {
            return false;
        };
        let typed = record(
            &self.native_state.value_type,
            fields
                .iter()
                .map(|f| (f.name(), f.value().clone()))
                .collect(),
        );
        conduit_plot::rust_binding::validate_native_contracts(
            &typed,
            &self.native_state.value_contracts,
        )
        .and_then(|()| {
            conduit_plot::rust_binding::validate_native_invariants(
                &typed,
                &self.native_state.invariants,
            )
        })
        .is_ok()
    }
    fn ty(&self, name: &str) -> &StructuredInfoType {
        &self.types.iter().find(|(n, _)| n == name).unwrap().1
    }
    fn basis(&self, revision: &str) -> StructuredInfoValue {
        let ty = self.ty("LanguageParserBasis");
        record(
            ty,
            vec![
                ("text", text(field_type(ty, "text"), "utterance")),
                (
                    "source_revision",
                    text(field_type(ty, "source_revision"), "source/1"),
                ),
                (
                    "analysis_revision",
                    text(field_type(ty, "analysis_revision"), revision),
                ),
            ],
        )
    }
    fn initial(&mut self, count: u64) -> StructuredInfoValue {
        let ty = self.ty("LanguageParserBegin");
        let input = record(
            ty,
            vec![
                ("basis", self.basis("analysis/1")),
                ("default_relation", self.relation("dep")),
                ("token_count", number(field_type(ty, "token_count"), count)),
            ],
        );
        evaluate(&mut self.initialize, &input)
    }
    fn action(&self, tag: &str) -> StructuredInfoValue {
        unit_variant(self.ty("LanguageParserAction"), tag)
    }
    fn relation(&self, relation: &str) -> StructuredInfoValue {
        let ty = self.ty("LanguageParserRelation");
        record(
            ty,
            vec![
                ("base", unit_variant(field_type(ty, "base"), relation)),
                ("subtype", text(field_type(ty, "subtype"), "")),
            ],
        )
    }
    fn project(&mut self, state: &StructuredInfoValue, dependent: u64) -> StructuredInfoValue {
        let ty = self.ty("LanguageParserArcQuery");
        let query = record(
            ty,
            vec![
                ("state", state.clone()),
                ("dependent", number(field_type(ty, "dependent"), dependent)),
            ],
        );
        evaluate(&mut self.arc, &query)
    }
    fn step(
        &mut self,
        state: &StructuredInfoValue,
        action: &str,
        relation: &str,
        revision: &str,
    ) -> StructuredInfoValue {
        let ty = self.ty("LanguageParserRequest");
        let mut input = record(
            ty,
            vec![
                ("basis", self.basis(revision)),
                ("state", state.clone()),
                ("action", self.action(action)),
                ("relation", self.relation(relation)),
            ],
        );
        for evaluator in &mut self.steps {
            input = evaluate(evaluator, &input);
        }
        input
    }
}
fn evaluate(
    evaluator: &mut PreparedPortableExpressionEvaluator,
    input: &StructuredInfoValue,
) -> StructuredInfoValue {
    StructuredInfoValue::from_canonical_bytes(
        evaluator
            .evaluate(&input.canonical_bytes().unwrap())
            .unwrap(),
    )
    .unwrap()
}
fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn case_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|c| c.tag() == name)
        .unwrap()
        .payload_type()
}
fn record(
    ty: &StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .into_iter()
            .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn text(ty: &StructuredInfoType, value: &str) -> StructuredInfoValue {
    if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
        return StructuredInfoValue::nominal(ty.clone(), text(representation, value)).unwrap();
    }
    StructuredInfoValue::leaf(ty.clone(), value.as_bytes().to_vec()).unwrap()
}
fn number(ty: &StructuredInfoType, value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), value.to_le_bytes().to_vec()).unwrap()
}
fn unit_variant(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    StructuredInfoValue::variant(
        ty.clone(),
        tag,
        StructuredInfoValue::leaf(case_type(ty, tag).clone(), vec![]).unwrap(),
    )
    .unwrap()
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn count(value: &StructuredInfoValue) -> u64 {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("number")
    };
    u64::from_le_bytes(bytes.try_into().unwrap())
}
fn accepted(value: &StructuredInfoValue) -> bool {
    let StructuredInfoValueShape::Leaf(bytes) = field(value, "accepted").shape() else {
        panic!("bool")
    };
    bytes == [1]
}
fn index(value: &StructuredInfoValue, i: usize) -> &StructuredInfoValue {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("collection")
    };
    &values[i]
}
fn tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("variant")
    };
    tag
}

#[test]
fn root_vocative_and_reduce_are_source_owned_transitions() {
    let mut f = Fixture::new();
    let mut state = f.initial(2);
    for (action, relation) in [
        ("right_arc", "root"),
        ("right_arc", "vocative"),
        ("reduce", "dep"),
        ("reduce", "dep"),
    ] {
        let result = f.step(&state, action, relation, "analysis/1");
        assert!(accepted(&result));
        state = field(&result, "state").clone();
    }
    assert_eq!(count(field(&state, "unread")), 2);
    assert_eq!(count(field(&state, "depth")), 1);
    assert_eq!(count(index(field(&state, "heads"), 0)), 4);
    assert_eq!(count(index(field(&state, "heads"), 1)), 0);
    assert_eq!(tag(field(field(&state, "relation1"), "base")), "vocative");
    let projected = f.project(&state, 1);
    assert_eq!(tag(&projected), "assigned");
    let StructuredInfoValueShape::Variant { payload, .. } = projected.shape() else {
        panic!("arc")
    };
    let basis = field(payload, "basis");
    let token = |ordinal| {
        conduit_language::LanguageAnalysisTokenRef::new(
            conduit_language::LanguageAnalysisRevisionId::from_structured(
                field(basis, "analysis_revision").clone(),
            )
            .unwrap(),
            conduit_language::LinguisticTokenIdentity::new(
                ordinal,
                conduit_language::LanguageTextId::from_structured(field(basis, "text").clone())
                    .unwrap(),
                conduit_language::LanguageTextRevisionId::from_structured(
                    field(basis, "source_revision").clone(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let governor = token(count(field(payload, "head")));
    let arc = conduit_language::LanguageDependencyArc::new(
        token(count(field(payload, "dependent"))),
        conduit_language::LanguageDependencyHead::token(
            governor.revision().clone(),
            governor.token().clone(),
        )
        .unwrap(),
        conduit_language::LanguageDependencyRelation::new(
            conduit_language::LanguageUniversalDependencyRelation::from_structured(
                field(field(payload, "relation"), "base").clone(),
            )
            .unwrap(),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(*arc.dependent().token().ordinal(), 1);
    assert_eq!(
        arc.dependent().token().text_identity(),
        &conduit_language::LanguageTextId::new("utterance".into()).unwrap()
    );
}
#[test]
fn headless_shift_can_receive_a_left_arc_before_input_end() {
    let mut f = Fixture::new();
    let state = f.initial(3);
    let shifted = f.step(&state, "shift", "dep", "analysis/1");
    assert!(accepted(&shifted));
    let linked = f.step(field(&shifted, "state"), "left_arc", "nsubj", "analysis/1");
    assert!(accepted(&linked));
    let state = field(&linked, "state");
    assert_eq!(count(field(state, "unread")), 1);
    assert_eq!(count(index(field(state, "heads"), 0)), 1);
}
#[test]
fn illegal_or_stale_proposals_preserve_state() {
    let mut f = Fixture::new();
    let state = f.initial(1);
    for (action, relation, revision) in [
        ("reduce", "dep", "analysis/1"),
        ("left_arc", "obj", "analysis/1"),
        ("right_arc", "vocative", "analysis/1"),
        ("shift", "dep", "analysis/2"),
    ] {
        let result = f.step(&state, action, relation, revision);
        assert!(!accepted(&result));
        assert_eq!(field(&result, "state"), &state);
    }
    let shifted = f.step(&state, "shift", "dep", "analysis/1");
    let state = field(&shifted, "state");
    for action in ["shift", "reduce", "left_arc", "right_arc"] {
        let result = f.step(state, action, "dep", "analysis/1");
        assert!(!accepted(&result));
        assert_eq!(field(&result, "state"), state);
    }
}

fn replace(
    value: &StructuredInfoValue,
    name: &str,
    replacement: StructuredInfoValue,
) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    record(
        value.value_type(),
        fields
            .iter()
            .map(|field| {
                (
                    field.name(),
                    if field.name() == name {
                        replacement.clone()
                    } else {
                        field.value().clone()
                    },
                )
            })
            .collect(),
    )
}

#[test]
fn commitment_and_finite_token_bounds_refuse_mutation() {
    let mut f = Fixture::new();
    let state = f.initial(4);
    assert_eq!(tag(&f.project(&state, 0)), "unassigned");
    let shifted = f.step(&state, "shift", "dep", "analysis/1");
    let state = field(&shifted, "state");
    let initial = f.initial(4);
    let root = f.step(&initial, "right_arc", "root", "analysis/1");
    let root_state = field(&root, "state");
    let committed = replace(
        root_state,
        "committed",
        number(field(root_state, "committed").value_type(), 1),
    );
    let refused = f.step(&committed, "left_arc", "nsubj", "analysis/1");
    assert!(!accepted(&refused));
    assert_eq!(field(&refused, "state"), &committed);
    let mut state = state.clone();
    for _ in 1..4 {
        let result = f.step(&state, "shift", "dep", "analysis/1");
        assert!(accepted(&result));
        state = field(&result, "state").clone();
    }
    let refused = f.step(&state, "shift", "dep", "analysis/1");
    assert!(!accepted(&refused));
    assert_eq!(field(&refused, "state"), &state);
    let invalid = replace(
        &state,
        "token_count",
        number(field(&state, "token_count").value_type(), 5),
    );
    assert!(!f.native_ok(&invalid));
    let result = f.step(&invalid, "shift", "dep", "analysis/1");
    assert!(!accepted(&result));
}

#[test]
fn proposed_arc_that_closes_a_cycle_is_refused() {
    let mut f = Fixture::new();
    let state = f.initial(2);
    let shifted = f.step(&state, "shift", "dep", "analysis/1");
    let state = field(&shifted, "state");
    let heads = field(state, "heads");
    let StructuredInfoValueShape::Collection(values) = heads.shape() else {
        panic!("heads")
    };
    let mut changed = values.to_vec();
    changed[0] = number(changed[0].value_type(), 1);
    let state = replace(
        state,
        "heads",
        StructuredInfoValue::collection(heads.value_type().clone(), changed).unwrap(),
    );
    let refused = f.step(&state, "right_arc", "obj", "analysis/1");
    assert!(!accepted(&refused));
    assert_eq!(field(&refused, "state"), &state);
}

#[test]
fn invalid_raw_graph_is_refused_by_native_and_source_admission() {
    let mut f = Fixture::new();
    let state = f.initial(2);
    assert!(f.native_ok(&state));
    let heads = field(&state, "heads");
    let StructuredInfoValueShape::Collection(values) = heads.shape() else {
        panic!("heads")
    };
    let mut changed = values.to_vec();
    changed[0] = number(changed[0].value_type(), 4);
    let invalid = replace(
        &state,
        "heads",
        StructuredInfoValue::collection(heads.value_type().clone(), changed).unwrap(),
    );
    assert!(!f.native_ok(&invalid));
    let refused = f.step(&invalid, "right_arc", "root", "analysis/1");
    assert!(!accepted(&refused));
    assert_eq!(tag(field(&refused, "refusal")), "invalid_state");
    assert_eq!(field(&refused, "state"), &invalid);
}
