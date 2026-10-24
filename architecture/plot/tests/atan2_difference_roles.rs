//! The authored intrinsic role Type participates in the ordinary atan2 path.
use conduit_core::*;
use conduit_plot::*;

const SOURCE: &str = "type Pair = {\n y: TemperatureDelta\n x: TemperatureDelta\n}\nplot bearing (\n >> value: Pair\n result: MathematicalAngle >>\n) = (math/atan2(.y, .x))\n";

fn program() -> PortableExpressionProgram {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profiles).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "bearing", &profiles).unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("canonical expression program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

fn packet(program: &PortableExpressionProgram, y: &str, x: &str) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("pair")
    };
    let fields = fields
        .iter()
        .map(|f| {
            assert!(f.name() == "y" || f.name() == "x");
            assert!(
                matches!(f.value_type().shape(), StructuredInfoTypeShape::Leaf(k)
            if k.as_str() == temperature_delta_info_id())
            );
            let q = Quantity::parse_plot_literal(if f.name() == "y" { y } else { x }).unwrap();
            StructuredFieldValue::new(
                f.name(),
                StructuredInfoValue::leaf(f.value_type().clone(), q.encode().to_vec()).unwrap(),
            )
            .unwrap()
        })
        .collect();
    StructuredInfoValue::record(program.input_type.clone(), fields)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

#[test]
fn authored_temperature_delta_type_keeps_exact_scale_and_principal_turns() {
    let p = program();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let capacity = prepared.output_capacity();
    for (y, x, n, d) in [
        ("TemperatureDelta(9, °F)", "TemperatureDelta(5, K)", 1, 8),
        ("TemperatureDelta(-9, °F)", "TemperatureDelta(5, K)", -1, 8),
        ("TemperatureDelta(9, °F)", "TemperatureDelta(0, K)", 1, 4),
        ("TemperatureDelta(0, °F)", "TemperatureDelta(-5, K)", 1, 2),
    ] {
        let input = packet(&p, y, x);
        let actual = prepared.evaluate(&input).unwrap().to_vec();
        assert_eq!(p.evaluate(&input).unwrap(), actual);
        let mut work = ExactNumericWork::new(100_000_000);
        let angle = MathematicalAngle::decode(&actual, &mut work).unwrap();
        assert_eq!(angle.unit(), Unit::Turn);
        assert_eq!(
            angle.coordinate().as_rational(),
            Some(ExactRational::ratio(n, d, &mut work).unwrap())
        );
        assert_eq!(prepared.output_capacity(), capacity);
    }
}

#[test]
fn intrinsic_difference_type_rejects_point_and_wrong_family_bytes() {
    let ty = quantity_conversion::temperature_difference::source_type();
    for source in ["21°C", "1m"] {
        let q = Quantity::parse_plot_literal(source).unwrap();
        assert!(StructuredInfoValue::leaf(ty.clone(), q.encode().to_vec()).is_err());
    }
}
