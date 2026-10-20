//! Source typing and carrier custody; runtime numerical admission is separate.
use conduit_core::*;
use conduit_plot::*;
use conduit_semantic_catalog::*;

fn compile(output: &str, body: &str) -> Result<ExpandedCanonicalPlot, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = format!(
        "plot fold (\n >> value: Frequency near 0.001Hz\n result: {output} >>\n) = ({body})\n"
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).map_err(|e| format!("{e:?}"))?;
    expand_canonical_plot_for_authoring(&checked, "fold", &profile)
        .map(|authored| authored.expanded)
        .map_err(|e| format!("{e:?}"))
}

#[test]
fn physical_body_retains_qualified_ports_and_requires_a_certified_host() {
    let mut identities = Vec::new();
    for output in ["Frequency near 0.2Hz", "Frequency near 0.01Hz"] {
        let expanded = compile(output, "(. + 10000000000000000Hz) - 10000000000000000Hz").unwrap();
        let gear = &expanded.gears[0];
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("retained program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let PortableExpressionOperation::QualifiedQuantity {
            input,
            output,
            physical_input,
            value,
        } = &program.root.operation
        else {
            panic!("qualified computation must retain its physical body")
        };
        assert_eq!(input.value_kind, gear.inputs[0].value_kind);
        assert_eq!(output.value_kind, gear.outputs[0].value_kind);
        for (location, expected) in [
            (FrontValueLocation::Input(port_id("input")), input),
            (FrontValueLocation::Output(port_id("output")), output),
        ] {
            let retained = gear
                .semantic_contract
                .value_contracts()
                .iter()
                .find(|entry| entry.location == location)
                .unwrap();
            assert_eq!(&retained.contract, expected);
        }
        assert_eq!(
            physical_input,
            &StructuredInfoType::leaf(kind_id(FREQUENCY_INFO_ID)).unwrap()
        );
        assert_eq!(&value.value_type, physical_input);
        assert_ne!(&program.input_type, physical_input);
        assert_ne!(&program.output_type, &value.value_type);
        assert!(PreparedPortableExpressionEvaluator::new(&program).is_err());
        let bytes = program.canonical_bytes().unwrap();
        assert_eq!(
            PortableExpressionProgram::from_canonical_bytes(&bytes).unwrap(),
            program
        );
        identities.push(gear.kind_id.clone());
        let mut downgraded = program.clone();
        downgraded.root.operation = PortableExpressionOperation::ValueContract {
            input: Some(input.clone()),
            output: Some(output.clone()),
            value: value.clone(),
        };
        assert!(
            downgraded.canonical_bytes().is_err(),
            "ordinary predicate wrapping cannot replace qualified computation"
        );

        let mut wrong_carrier = program.clone();
        wrong_carrier.root.value_type = physical_input.clone();
        assert!(wrong_carrier.canonical_bytes().is_err());

        let mut forged = program.clone();
        let PortableExpressionOperation::QualifiedQuantity { physical_input, .. } =
            &mut forged.root.operation
        else {
            unreachable!()
        };
        *physical_input = StructuredInfoType::leaf(kind_id(DISTANCE_INFO_ID)).unwrap();
        assert!(forged.canonical_bytes().is_err());
    }
    assert_ne!(
        identities[0], identities[1],
        "consumer tolerance belongs to identity"
    );
}

#[test]
fn arithmetic_cannot_escape_qualification_or_change_physical_family() {
    for (output, body) in [
        ("Frequency", ". + 1Hz"),
        ("Distance near 0.2m", ". + 1Hz"),
        ("Frequency near 0.2Hz", ". + 1m"),
    ] {
        assert!(compile(output, body).is_err(), "{output}: {body}");
    }
}
