#![allow(dead_code)]
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

pub struct Fixture {
    types: Vec<(String, StructuredInfoType)>,
    initialize: PreparedPortableExpressionEvaluator,
    steps: Vec<PreparedPortableExpressionEvaluator>,
    arc: PreparedPortableExpressionEvaluator,
    native_state: conduit_plot::CheckedNativeType,
    checked: conduit_plot::CheckedSyntaxDocument,
}
impl Fixture {
    pub fn new() -> Self {
        let source = format!(
            "{}\n{}\n{}\n{}\n{}",
            include_str!("../../identity.conduit"),
            include_str!("../../types.conduit"),
            include_str!("../../parser.conduit"),
            include_str!("../../parser_beam.conduit"),
            include_str!("../../parser_scorer.conduit")
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
            checked: checked.clone(),
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
    pub fn native_ok(&self, state: &StructuredInfoValue) -> bool {
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
    pub fn prepare(&self, name: &str) -> PreparedPortableExpressionEvaluator {
        let expanded =
            expand_canonical_plot_for_authoring(&self.checked, name, &ProfileCatalog::new())
                .unwrap_or_else(|e| panic!("{name}: {e:?}"))
                .expanded;
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("program")
        };
        PreparedPortableExpressionEvaluator::new(
            &PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
        )
        .unwrap_or_else(|e| panic!("{name} preparation: {e:?}"))
    }
    pub fn run(&self, name: &str, input: &StructuredInfoValue) -> StructuredInfoValue {
        evaluate(&mut self.prepare(name), input)
    }
    pub fn ty(&self, name: &str) -> &StructuredInfoType {
        &self.types.iter().find(|(n, _)| n == name).unwrap().1
    }
    pub fn basis(&self, revision: &str) -> StructuredInfoValue {
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
    pub fn initial(&mut self, count: u64) -> StructuredInfoValue {
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
    pub fn action(&self, tag: &str) -> StructuredInfoValue {
        unit_variant(self.ty("LanguageParserAction"), tag)
    }
    pub fn relation(&self, relation: &str) -> StructuredInfoValue {
        let ty = self.ty("LanguageParserRelation");
        record(
            ty,
            vec![
                ("base", unit_variant(field_type(ty, "base"), relation)),
                ("subtype", text(field_type(ty, "subtype"), "")),
            ],
        )
    }
    pub fn project(&mut self, state: &StructuredInfoValue, dependent: u64) -> StructuredInfoValue {
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
    pub fn step(
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
pub fn evaluate(
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
pub fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
pub fn case_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|c| c.tag() == name)
        .unwrap()
        .payload_type()
}
pub fn record(
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
pub fn text(ty: &StructuredInfoType, value: &str) -> StructuredInfoValue {
    if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
        return StructuredInfoValue::nominal(ty.clone(), text(representation, value)).unwrap();
    }
    StructuredInfoValue::leaf(ty.clone(), value.as_bytes().to_vec()).unwrap()
}
pub fn number(ty: &StructuredInfoType, value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), value.to_le_bytes().to_vec()).unwrap()
}
pub fn unit_variant(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    StructuredInfoValue::variant(
        ty.clone(),
        tag,
        StructuredInfoValue::leaf(case_type(ty, tag).clone(), vec![]).unwrap(),
    )
    .unwrap()
}
pub fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
pub fn count(value: &StructuredInfoValue) -> u64 {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("number")
    };
    u64::from_le_bytes(bytes.try_into().unwrap())
}
pub fn accepted(value: &StructuredInfoValue) -> bool {
    let StructuredInfoValueShape::Leaf(bytes) = field(value, "accepted").shape() else {
        panic!("bool")
    };
    bytes == [1]
}
pub fn index(value: &StructuredInfoValue, i: usize) -> &StructuredInfoValue {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("collection")
    };
    &values[i]
}
pub fn tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("variant")
    };
    tag
}

pub fn replace(
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
