use conduit_core::*;
use conduit_language::*;
use conduit_plot::rust_binding::{
    validate_native_contracts, validate_native_invariants, NativeRustBinding,
};
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
fn record(
    ty: &StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .into_iter()
            .map(|(name, v)| StructuredFieldValue::new(name, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn field<'a>(v: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = v.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn replace(v: &StructuredInfoValue, name: &str, value: StructuredInfoValue) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = v.shape() else {
        panic!("record")
    };
    record(
        v.value_type(),
        fields
            .iter()
            .map(|f| {
                (
                    f.name(),
                    if f.name() == name {
                        value.clone()
                    } else {
                        f.value().clone()
                    },
                )
            })
            .collect(),
    )
}
fn number(ty: &StructuredInfoType, n: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), n.to_le_bytes().to_vec()).unwrap()
}
fn evaluate(
    checked: &conduit_plot::CheckedSyntaxDocument,
    name: &str,
    input: &StructuredInfoValue,
) -> StructuredInfoValue {
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        checked,
        name,
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    assert_eq!(expanded.gears.len(), 1, "single Source expression only");
    let ConfigurationValue::Text(hex) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let p = conduit_plot::PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let mut e = conduit_plot::PreparedPortableExpressionEvaluator::new(&p).unwrap();
    StructuredInfoValue::from_canonical_bytes(
        e.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
    )
    .unwrap()
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let fixture: &str = &args[0];
    let source = args[1..]
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let syntax = conduit_plot::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new()).unwrap();
    let native = checked
        .native_types
        .iter()
        .find(|t| t.name == "LanguageParserIndependentProtectedAdmission")
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    let decode = |key: &str| {
        StructuredInfoValue::from_canonical_bytes(
            &serde_json::from_value::<Vec<u8>>(json[key].clone()).unwrap(),
        )
        .unwrap()
    };
    let context = if json.get("native_context_bytes").is_some() {
        LanguageParserProtectedProjectionContext::from_structured(decode("native_context_bytes"))
            .unwrap()
    } else {
        let bytes: Vec<u8> = serde_json::from_value(json["stable_fact_bytes"][0].clone()).unwrap();
        let fact = LanguageParserJointStableFact::from_structured(
            StructuredInfoValue::from_canonical_bytes(&bytes).unwrap(),
        )
        .unwrap();
        let q = fact.query();
        let dep = *q.dependent() as usize;
        let head = q.beam().candidate0().parser().state().heads()[dep] as usize;
        let dependent = q.beam().lexical().tape().tokens()[dep].clone();
        let governor = if head < 4 {
            q.beam().lexical().tape().tokens()[head].clone()
        } else {
            dependent.clone()
        };
        LanguageParserProtectedProjectionContext::new(dependent, fact, governor).unwrap()
    };
    let fact = context.fact();
    let q = fact.query();
    let dep = *q.dependent() as usize;
    let state = q.beam().candidate0().parser().state();
    let head = state.heads()[dep];
    let token = |i: usize| {
        LanguageAnalysisTokenRef::new(
            q.beam().basis().analysis_revision().clone(),
            q.beam().lexical().tape().tokens()[i].identity().clone(),
        )
        .unwrap()
    };
    let governor = if head == 4 {
        LanguageDependencyHead::Root
    } else {
        let t = token(head as usize);
        LanguageDependencyHead::token(t.revision().clone(), t.token().clone()).unwrap()
    };
    let rel = [
        state.relation0(),
        state.relation1(),
        state.relation2(),
        state.relation3(),
    ][dep];
    let subtype = if rel.subtype().get().is_empty() {
        None
    } else {
        Some(LanguageDependencySubtype::new(rel.subtype().get().into()).unwrap())
    };
    let arc = LanguageDependencyArc::new(
        token(dep),
        governor,
        LanguageDependencyRelation::new(*rel.base(), subtype).unwrap(),
    )
    .unwrap();
    let admission_type = LanguageParserStableDependencyAdmission::semantic_type().unwrap();
    let admission = LanguageParserStableDependencyAdmission::from_structured(record(
        &admission_type,
        vec![
            ("fact", fact.clone().into_structured().unwrap()),
            ("arc", arc.into_structured().unwrap()),
            ("head", number(field_type(&admission_type, "head"), head)),
            ("subtype", rel.subtype().clone().into_structured().unwrap()),
        ],
    ))
    .unwrap();
    let edge = evaluate(
        &checked,
        "language-parser-protected-edge-projection",
        &context.clone().into_structured().unwrap(),
    );
    let previous = evaluate(&checked, "language-parser-protected-set-initialize", &edge);
    let previous = LanguageParserProtectedSetProposal::from_structured(previous).unwrap();
    let insert_type = LanguageParserProtectedInsertContext::semantic_type().unwrap();
    let insert = LanguageParserProtectedInsertContext::from_structured(record(
        &insert_type,
        vec![
            ("context", context.into_structured().unwrap()),
            ("previous", previous.into_structured().unwrap()),
        ],
    ))
    .unwrap();
    let projected = evaluate(
        &checked,
        "language-parser-protected-insert-projection",
        &insert.clone().into_structured().unwrap(),
    );
    let output = evaluate(&checked, "language-parser-protected-set-upsert", &projected);
    let value = record(
        &native.value_type,
        vec![
            ("admission", admission.into_structured().unwrap()),
            ("insert", insert.into_structured().unwrap()),
            ("output", output),
        ],
    );
    validate_native_contracts(&value, &native.value_contracts).unwrap();
    validate_native_invariants(&value, &native.invariants).unwrap();
    let output = field(&value, "output");
    let key = format!("edge{dep}");
    let edge = field(output, &key);
    for (name, replacement) in [
        ("head", number(field_type(edge.value_type(), "head"), 4)),
        (
            "dependent_choice",
            number(field_type(edge.value_type(), "dependent_choice"), 4),
        ),
        (
            "head_choice",
            number(field_type(edge.value_type(), "head_choice"), 4),
        ),
    ] {
        let changed = replace(
            &value,
            "output",
            replace(output, &key, replace(edge, name, replacement)),
        );
        assert!(
            validate_native_invariants(&changed, &native.invariants).is_err(),
            "must refuse {name}"
        );
    }
    println!("full actual VOC{dep} fact and Source-expression output admitted; three changed head/choice outputs refused; no ordinary Plan or played claim");
    std::fs::write(
        format!("work/protection/independent-admission-actual-voc{dep}.bin"),
        value.canonical_bytes().unwrap(),
    )
    .unwrap();
}
