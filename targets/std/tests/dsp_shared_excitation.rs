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
use preparation::field;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "common_shared_dsp/source.rs"]
mod shared_source;
use shared_source::{components, source, value_profile};

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
            for seed_word in [0_i32, 7, 65535] {
                let seed = StructuredInfoValue::record(
                    program.input_type.clone(),
                    vec![StructuredFieldValue::new(
                        "word",
                        StructuredInfoValue::leaf(
                            fields[0].value_type().clone(),
                            seed_word.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    )
                    .unwrap()],
                )
                .unwrap();
                let mut input = program.evaluate(&seed.canonical_bytes().unwrap()).unwrap();
                let mut reference_word = seed_word as u64;
                for frame in 0_i32..10 {
                    let (plan, front, pair) = preparation::prepare(
                        &source,
                        "SharedExcitationStage",
                        "shared-excitation-frame",
                    );
                    assert!(plan.fragments[0].placements.len() <= 64);
                    let encoded = kernel::run(plan, &front, pair, vec![], input, false).unwrap();
                    let result = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
                    let StructuredInfoValueShape::Variant { tag, payload } = result.shape() else {
                        panic!("context")
                    };
                    assert_eq!(tag, "context");
                    let cursor = field(payload, "cursor");
                    reference_word = (reference_word * 25173 + 13849) & 65535;
                    let level = if frame < 8 { frame * 32 } else { 256 };
                    let sample = (((reference_word as i64 - 32768) * i64::from(level)) / 256)
                        .clamp(-32767, 32767);
                    for (name, expected) in [
                        ("word", reference_word as i32),
                        ("frame", frame + 1),
                        ("sample", sample as i32),
                    ] {
                        let value = field(&cursor, name);
                        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                            panic!("integer")
                        };
                        assert_eq!(i32::from_le_bytes(bytes.try_into().unwrap()), expected);
                    }
                    let remaining = field(&cursor, "remaining");
                    let StructuredInfoValueShape::Leaf(bytes) = remaining.shape() else {
                        panic!("remaining")
                    };
                    assert_eq!(
                        u32::from_le_bytes(bytes.try_into().unwrap()),
                        159 - frame as u32
                    );
                    // Actual committed output supplies the next frame's state.
                    input = encoded;
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
