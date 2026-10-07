use conduit_core::*;
use conduit_plot::*;
fn collection(ty: &StructuredInfoType) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), collection(representation)).unwrap()
        }
        StructuredInfoTypeShape::Collection { element, length } => StructuredInfoValue::collection(
            ty.clone(),
            (0..length)
                .map(|i| StructuredInfoValue::leaf(element.clone(), vec![i as u8]).unwrap())
                .collect(),
        )
        .unwrap(),
        _ => panic!("fixed collection"),
    }
}
#[test]
fn adjacent_pure_stages_preserve_exact_large_canonical_contracts_and_refuse_oversize() {
    let source="type Wide = collection U8 = 128\ntype Wrapped = {\n value: Wide\n}\nplot wrap (\n >> input: Wide\n output: Wrapped >>\n) = ({value: .})\nplot unwrap (\n >> input: Wrapped\n output: Wide >>\n) = (.value)\nplot proof (\n >> input: Wide\n output: Wide >>\n) {\n a: wrap\n b: unwrap\n input >> a.input\n a.output >> b.input\n b.output >> output\n}\n";
    let startup = StartupCatalog::new();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let wide = &checked
        .native_types
        .iter()
        .find(|t| t.name == "Wide")
        .unwrap()
        .value_type;
    let original = collection(wide).canonical_bytes().unwrap();
    assert!(original.len() > 64);
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "proof", &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 2);
    let mut encoded = original.clone();
    for gear in &expanded.expanded.gears {
        let ConfigurationValue::Text(program) = &gear.configuration[0].value else {
            panic!("checked program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(program).unwrap();
        let front = gear.checked_front();
        for (location, expected) in [
            (
                FrontValueLocation::Input(port_id("input")),
                program.maximum_prepared_input_bytes().unwrap(),
            ),
            (
                FrontValueLocation::Output(port_id("output")),
                program.maximum_prepared_output_bytes().unwrap(),
            ),
        ] {
            let contract = &front
                .value_contracts()
                .iter()
                .find(|c| c.location == location)
                .unwrap()
                .contract;
            assert_eq!(contract.maximum_bytes, expected);
            assert!(expected > 64);
        }
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let oversized = vec![0; program.maximum_prepared_input_bytes().unwrap() as usize + 1];
        assert!(evaluator.evaluate(&oversized).is_err());
        encoded = evaluator.evaluate(&encoded).unwrap().to_vec();
    }
    assert_eq!(encoded, original);
}
