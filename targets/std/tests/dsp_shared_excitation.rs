//! First shared component frame; full speech/music and partition acceptance is separate.
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common_acoustic_value/kernel.rs"]
mod kernel;
#[path = "common_acoustic_reference/mod.rs"]
mod preparation;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn field(value: &StructuredInfoValue, name: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record");
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
        .clone()
}

fn source() -> String {
    let units = [
        include_str!("../../../semantics/audio/dsp/seeded-noise.conduit"),
        include_str!("../../../semantics/audio/dsp/envelope.conduit"),
        include_str!("../../../semantics/audio/dsp/gain.conduit"),
        include_str!("fixtures/shared-excitation-frame.conduit"),
    ];
    let mut imports = Vec::new();
    let mut declarations = Vec::new();
    let mut plots = Vec::new();
    for unit in units {
        let (prefix, body) = unit.split_once("plot ").unwrap();
        for line in prefix.lines() {
            if line.starts_with("with ") {
                imports.push(line);
            } else {
                declarations.push(line);
            }
        }
        plots.push(format!("plot {body}"));
    }
    for (unit, entry, value_entry) in components() {
        plots.push(value_profile(unit, entry, value_entry));
    }
    format!(
        "{}\n{}\n{}\n",
        imports.join("\n"),
        declarations.join("\n"),
        plots.join("\n")
    )
}

fn components() -> [(&'static str, &'static str, &'static str); 3] {
    [
        (
            include_str!("../../../semantics/audio/dsp/seeded-noise.conduit"),
            "dsp/noise-step-candidate",
            "dsp/noise-step-value",
        ),
        (
            include_str!("../../../semantics/audio/dsp/envelope.conduit"),
            "dsp/envelope",
            "dsp/envelope-value",
        ),
        (
            include_str!("../../../semantics/audio/dsp/gain.conduit"),
            "dsp/gain",
            "dsp/gain-value",
        ),
    ]
}

// This fixture derives an explicit Value profile from one owned pure equation.
// The source equation and Type declarations are never reimplemented in Rust.
fn value_profile(unit: &str, entry: &str, value_entry: &str) -> String {
    let marker = format!("plot {entry} (");
    let start = unit.rfind(&marker).unwrap();
    let definition = &unit[start..];
    assert!(!definition.contains("\nplot "));
    assert_eq!(definition.matches("...|").count(), 2);
    definition
        .replacen(&marker, &format!("plot {value_entry} ("), 1)
        .replace("...|", "")
}

fn program(unit: &str, entry: &str) -> PortableExpressionProgram {
    let checked =
        check_syntax_document(&parse_syntax_document(unit), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

#[test]
fn value_profiles_retain_the_exact_owned_dsp_programs() {
    for (unit, entry, value_entry) in components() {
        let derived = format!("{unit}\n{}", value_profile(unit, entry, value_entry));
        assert_eq!(
            program(unit, entry).canonical_hex().unwrap(),
            program(&derived, value_entry).canonical_hex().unwrap()
        );
    }
}

#[test]
fn shared_noise_envelope_and_gain_execute_as_one_checked_child() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = source();
            let seed_source = source
                .split("plot shared-excitation-unpack (")
                .next()
                .unwrap()
                .lines()
                .filter(|line| !line.starts_with("with "))
                .collect::<Vec<_>>()
                .join("\n");
            let checked =
                check_syntax_document(&parse_syntax_document(&seed_source), &StartupCatalog::new())
                    .unwrap();
            let start = expand_canonical_plot_for_authoring(
                &checked,
                "shared-excitation-start",
                &ProfileCatalog::new(),
            )
            .unwrap();
            let ConfigurationValue::Text(encoded) = &start.expanded.gears[0].configuration[0].value
            else {
                panic!("checked Source program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
                panic!("seed")
            };
            let seed = StructuredInfoValue::record(
                program.input_type.clone(),
                vec![StructuredFieldValue::new(
                    "word",
                    StructuredInfoValue::leaf(
                        fields[0].value_type().clone(),
                        7_i32.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap()],
            )
            .unwrap();
            let input = program.evaluate(&seed.canonical_bytes().unwrap()).unwrap();
            println!("shared seed canonical bytes={}", input.len());
            let (plan, front, pair) =
                preparation::prepare(&source, "SharedExcitationStage", "shared-excitation-frame");
            assert!(plan.fragments[0].placements.len() <= 64);
            let result = kernel::run(plan, &front, pair, vec![], input, false).unwrap();
            let result = StructuredInfoValue::from_canonical_bytes(&result).unwrap();
            let StructuredInfoValueShape::Variant { tag, payload } = result.shape() else {
                panic!("context")
            };
            assert_eq!(tag, "context");
            let cursor = field(payload, "cursor");
            for (name, expected) in [
                ("word", (7 * 25173 + 13849) % 65536),
                ("frame", 1),
                ("sample", 0),
            ] {
                let value = field(&cursor, name);
                let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                    panic!("integer")
                };
                assert_eq!(i32::from_le_bytes(bytes.try_into().unwrap()), expected);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
