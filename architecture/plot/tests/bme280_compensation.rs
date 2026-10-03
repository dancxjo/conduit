use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
const SOURCE: &str = concat!(
    include_str!("../../../plots/device-protocols/bme280-compensation.conduit"),
    "\n",
    include_str!("../../../plots/device-protocols/bme280-calibration.conduit"),
);
fn program(entry: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn input(p: &PortableExpressionProgram, values: &[(&str, i128)]) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        p.input_type.clone(),
        values
            .iter()
            .map(|(name, value)| {
                let ty = fields
                    .iter()
                    .find(|f| f.name() == *name)
                    .unwrap()
                    .value_type()
                    .clone();
                StructuredFieldValue::new(
                    *name,
                    StructuredInfoValue::leaf(ty, value.to_le_bytes().to_vec()).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn scalar(bytes: &[u8]) -> i128 {
    i128::from_le_bytes(bytes.try_into().unwrap())
}

#[test]
fn every_compensation_plot_checks_and_expands() {
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    for plot in &checked.plots {
        expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new())
            .unwrap_or_else(|error| panic!("{}: {error:?}", plot.name));
    }
}

#[test]
fn published_temperature_pressure_vector_is_carried_through_exact_stages() {
    let temp = program("bme280-fine-temperature");
    let fine = temp
        .evaluate(&input(
            &temp,
            &[("raw", 519888), ("t1", 27504), ("t2", 26435), ("t3", -1000)],
        ))
        .unwrap();
    assert_eq!(scalar(&fine), 128422);
    assert_eq!(
        scalar(
            &program("bme280-temperature-centidegrees")
                .evaluate(&fine)
                .unwrap()
        ),
        2508
    );
    let polynomial = program("bme280-pressure-polynomial");
    let raw = input(
        &polynomial,
        &[
            ("raw", 415148),
            ("fine", 128422),
            ("p1", 36477),
            ("p2", -10685),
            ("p3", 3024),
            ("p4", 2855),
            ("p5", 140),
            ("p6", -7),
            ("p7", 15500),
            ("p8", -14600),
            ("p9", 6000),
        ],
    );
    let mut current = polynomial.evaluate(&raw).unwrap();
    for stage in [
        "bme280-pressure-division",
        "bme280-pressure-quotient",
        "bme280-pressure-q24-8",
    ] {
        let p = program(stage);
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let capacity = prepared.output_capacity();
        for _ in 0..100 {
            assert_eq!(
                prepared.evaluate(&current).unwrap(),
                p.evaluate(&current).unwrap()
            );
        }
        assert_eq!(prepared.output_capacity(), capacity);
        current = p.evaluate(&current).unwrap();
    }
    assert_eq!(scalar(&current), 25767233);
}

#[test]
fn zero_pressure_denominator_and_humidity_clamps_are_explicit() {
    let invalid = program("bme280-pressure-invalid-calibration");
    let values = [
        ("p7", 0),
        ("p8", 0),
        ("p9", 0),
        ("numerator", 100),
        ("denominator", 0),
    ];
    assert_eq!(invalid.evaluate(&input(&invalid, &values)).unwrap(), [1]);
    let quotient = program("bme280-pressure-quotient");
    assert!(quotient.evaluate(&input(&quotient, &values)).is_ok());
    let clamp = program("bme280-humidity-q22-10");
    for (raw, expected) in [
        (-1_i128, 0),
        (0, 0),
        (4096, 1),
        (419430400, 102400),
        (419430401, 102400),
    ] {
        assert_eq!(
            scalar(&clamp.evaluate(&raw.to_le_bytes()).unwrap()),
            expected
        );
    }
}

#[test]
fn humidity_carries_calibration_then_applies_correction_and_clamp() {
    let p = program("bme280-humidity-polynomial");
    let input = input(
        &p,
        &[
            ("raw", 30000),
            ("fine", 128422),
            ("h1", 75),
            ("h2", 362),
            ("h3", 0),
            ("h4", 315),
            ("h5", 50),
            ("h6", 30),
        ],
    );
    let polynomial = p.evaluate(&input).unwrap();
    let corrected = program("bme280-humidity-correction")
        .evaluate(&polynomial)
        .unwrap();
    let humidity = program("bme280-humidity-q22-10")
        .evaluate(&corrected)
        .unwrap();
    assert_eq!(scalar(&humidity), 55588);
}

fn collection(ty: conduit_core::StructuredInfoType, octets: &[u8]) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Collection { element, .. } = ty.shape() else {
        panic!("collection")
    };
    let children = octets
        .iter()
        .map(|byte| StructuredInfoValue::leaf(element.clone(), vec![*byte]).unwrap())
        .collect();
    StructuredInfoValue::collection(ty, children).unwrap()
}

#[test]
fn wire_calibration_retains_unsigned_and_negative_packed_coefficients() {
    let p = program("bme280-decode-calibration");
    let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
        panic!("calibration")
    };
    let ty = |name| {
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let mut tp = [0_u8; 24];
    let coefficients = [
        65535_i32, -32768, 32767, 65535, -1, -32768, 32767, 0, 1, -7, -14600, 6000,
    ];
    for (pair, coefficient) in tp.as_chunks_mut::<2>().0.iter_mut().zip(coefficients) {
        pair.copy_from_slice(&(coefficient as u16).to_le_bytes());
    }
    // h2=-32768, h3=255, h4=-2048, h5=-1, h6=-128.
    let value = StructuredInfoValue::record(
        p.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "temperature_pressure",
                collection(ty("temperature_pressure"), &tp),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "humidity_first",
                StructuredInfoValue::leaf(ty("humidity_first"), vec![255]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "humidity",
                collection(ty("humidity"), &[0, 128, 255, 128, 240, 255, 128]),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let output = p.evaluate(&value).unwrap();
    let decoded = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
    let conduit_core::StructuredInfoValueShape::Record(fields) = decoded.shape() else {
        panic!("fields")
    };
    let get = |name| {
        let f = fields.iter().find(|f| f.name() == name).unwrap();
        let conduit_core::StructuredInfoValueShape::Leaf(bytes) = f.value().shape() else {
            panic!("integer")
        };
        scalar(bytes)
    };
    for (name, expected) in [
        ("t1", 65535),
        ("t2", -32768),
        ("p1", 65535),
        ("p2", -1),
        ("h1", 255),
        ("h2", -32768),
        ("h3", 255),
        ("h4", -2048),
        ("h5", -1),
        ("h6", -128),
    ] {
        assert_eq!(get(name), expected, "{name}");
    }
    let (mut prepared, storage) =
        allocation_probe::observe(|| PreparedPortableExpressionEvaluator::new(&p).unwrap());
    assert!(storage.peak_bytes < 1024 * 1024, "{storage:?}");
    let (_, play) = allocation_probe::observe(|| {
        for _ in 0..1000 {
            assert_eq!(prepared.evaluate(&value).unwrap(), output);
        }
    });
    assert_eq!((play.allocations, play.reallocations), (0, 0), "{play:?}");
}

#[test]
fn wire_sample_masks_reserved_nibbles_and_retains_disabled_sentinels() {
    let p = program("bme280-decode-sample");
    let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
        panic!("wire record")
    };
    let bytes = StructuredInfoValue::record(
        p.input_type.clone(),
        vec![StructuredFieldValue::new(
            "wire",
            collection(
                fields[0].value_type().clone(),
                &[0x80, 0, 15, 0x80, 0, 15, 0x80, 0],
            ),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let output = p.evaluate(&bytes).unwrap();
    assert_eq!(
        program("bme280-sample-disabled").evaluate(&output).unwrap(),
        [1]
    );
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(prepared.evaluate(&bytes).unwrap(), output);
}

#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
