//! Differential Source semantics; the staged fixture is never a runtime lane.
pub(super) const PRODUCTION: &str =
    include_str!("../../../../plots/device-protocols/bme280-lifecycle.conduit");
pub(super) const ORACLE: &str = include_str!("staged_oracle.conduit");
use super::{fixture::*, *};
use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};

pub(super) fn checked(source: &str) -> conduit_plot::CheckedSyntaxDocument {
    let syntax = parse_syntax_document(source);
    assert!(
        syntax.diagnostics.is_empty(),
        "parse: {:?}",
        syntax.diagnostics
    );
    check_syntax_document(&syntax, &lifecycle_catalog()).expect("checked experimental Source")
}
fn replace(state: &StructuredInfoValue, name: &str, bytes: &[u8]) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = state.shape() else {
        panic!("state")
    };
    StructuredInfoValue::record(
        state.value_type().clone(),
        fields
            .iter()
            .map(|field| {
                StructuredFieldValue::new(
                    field.name(),
                    if field.name() == name {
                        StructuredInfoValue::leaf(
                            field.value().value_type().clone(),
                            bytes.to_vec(),
                        )
                        .unwrap()
                    } else {
                        field.value().clone()
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn compare(reference: &mut Fixture, direct: &mut Fixture, input: &[u8]) {
    match (
        reference.try_advance_bytes(input),
        direct.try_advance_bytes(input),
    ) {
        (Ok(expected), Ok(actual)) => assert_eq!(actual, expected, "canonical transition differs"),
        (Err(expected), Err(actual)) => assert_eq!(
            core::mem::discriminant(&actual),
            core::mem::discriminant(&expected),
            "refusal class differs: {actual:?} / {expected:?}"
        ),
        (expected, actual) => panic!("success/refusal differs: {expected:?} / {actual:?}"),
    }
}
fn events(f: &Fixture, now: u64) -> Vec<StructuredInfoValue> {
    let mut result = vec![
        f.tick(now),
        f.bus("completed", &[], now),
        f.bus("completed", &[0], now),
        f.bus("completed", &[96], now),
        f.bus("completed", &[1, 2], now),
        f.bus("short", &[7], now),
    ];
    for tag in [
        "device-error",
        "not-acknowledged",
        "arbitration-lost",
        "timed-out",
        "provider-lost",
        "unsupported",
        "stale-attachment",
        "refused",
    ] {
        result.push(f.bus(tag, &[], now));
    }
    for tag in [
        "unavailable",
        "provider-lost",
        "unsupported-deadline",
        "malformed",
        "timeout",
    ] {
        result.push(f.clock_failure(tag));
    }
    result
}

#[test]
#[ignore = "finite13294-case portable differential corpus; run explicitly"]
fn full_transition_boundary_corpus_matches_staged_oracle() {
    let candidate = PRODUCTION;
    let original = ORACLE;
    let reference_checked = checked(original);
    let direct_checked = checked(candidate);
    assert_ne!(
        reference_checked.source_document_id,
        direct_checked.source_document_id
    );
    for name in [
        "BmeProtocolBegin",
        "BmeProtocolState",
        "BmeProtocolEvent",
        "BmeProtocolTransition",
        "BmeProtocolAction",
    ] {
        let get = |document: &conduit_plot::CheckedSyntaxDocument| {
            document
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .unwrap()
                .clone()
        };
        assert_eq!(
            get(&reference_checked),
            get(&direct_checked),
            "external native contract changed: {name}"
        );
    }
    eprintln!("direct qualification stage=portable-decode");
    let reference_programs = transition_programs(&reference_checked);
    let direct_programs = transition_programs(&direct_checked);
    assert_eq!(reference_programs.len(), 12);
    assert_eq!(direct_programs.len(), 2);
    assert_eq!(
        reference_programs[0].input_type,
        direct_programs[0].input_type
    );
    assert_eq!(
        reference_programs.last().unwrap().output_type,
        direct_programs.last().unwrap().output_type
    );
    for (index, program) in direct_programs.iter().enumerate() {
        eprintln!("direct qualification stage=prepared-evaluator index={index}");
        PreparedPortableExpressionEvaluator::new(program)
            .expect("original prepared evaluator bounds");
        eprintln!("direct qualification stage=static-image original native4096/scopes64/invariants64/contracts64/occurrences4096");
        let generated = conduit_plot::generate_borrowed_static_structured_expression_with_work(
            &direct_checked,
            program,
            conduit_plot::rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            4096,
        )
        .expect("original static constructor/native bounds; do not enlarge");
        eprintln!(
            "direct static image computation_work={} generated_rust_bytes={}",
            generated.computation_work,
            generated.source.len()
        );
    }
    // Computed work is evidence, NOT a private-work grant. Actual selected
    // embedded Make/factory must independently admit it against original fuel.
    eprintln!("direct qualification stage=differential-matrix");
    let mut reference = Fixture::staged_oracle();
    let mut direct = Fixture::from_source(candidate);
    let count = visit_qualification_inputs(&mut reference, &mut direct, compare);
    let initial = reference.initial();
    let input = direct.input_bytes(initial, direct.tick(1));
    direct.advance_bytes(&input);
    let (_, allocations) = allocation_probe::observe(|| {
        for _ in 0..16 {
            direct.advance_bytes(&input);
        }
    });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    eprintln!("direct differential cases={count}; no Play/admission/physical proof");
}

/// One shared finite corpus/transcript for portable and emitted-image proofs.
pub(super) fn visit_qualification_inputs(
    reference: &mut Fixture,
    direct: &mut Fixture,
    mut visit: impl FnMut(&mut Fixture, &mut Fixture, &[u8]),
) -> usize {
    let initial = reference.initial();
    let mut count = 0usize;
    for phase in 0..=u8::MAX {
        let state = reference.with_phase(&initial, phase);
        for event in events(reference, 123) {
            let input = reference.input_bytes(state.clone(), event);
            visit(reference, direct, &input);
            count += 1;
        }
    }
    for phase in [0, 1, 2, 3, 7, 8, 9, 10, 11, 12, 13, 14, 15, 255] {
        for address in [8, 117, 118, 119] {
            for index in [0, 6, 7, 8, 22, 23, 24, 255] {
                for polls in [0, 6, 7, 8, 255] {
                    let state = reference.with_phase(&initial, phase);
                    let state = replace(&state, "address", &[address]);
                    let state = reference.with_index(&state, index);
                    let state = replace(&state, "polls", &[polls]);
                    for event in [
                        reference.tick(123),
                        reference.bus("completed", &[0], 123),
                        reference.clock_failure("provider-lost"),
                    ] {
                        let input = reference.input_bytes(state.clone(), event);
                        visit(reference, direct, &input);
                        count += 1;
                    }
                }
            }
        }
    }
    for phase in [0, 1, 2, 10, 11, 12, 14, 15, 255] {
        for time in [
            0,
            1,
            2,
            19,
            20,
            21,
            u64::MAX - 20,
            u64::MAX - 2,
            u64::MAX - 1,
            u64::MAX,
        ] {
            let state = reference.with_phase(&initial, phase);
            let state = replace(&state, "clock", &20u64.to_le_bytes());
            let state = replace(&state, "deadline", &20u64.to_le_bytes());
            for event in events(reference, time) {
                let input = reference.input_bytes(state.clone(), event);
                visit(reference, direct, &input);
                count += 1;
            }
        }
    }
    // Original event priority and projection guards must survive fieldwise
    // rewriting: malformed canonical input and arithmetic overflow are refusals.
    let overflow = reference.with_phase(&initial, 1);
    let input = reference.input_bytes(overflow, reference.bus("completed", &[], u64::MAX));
    assert!(reference.try_advance_bytes(&input).is_err());
    visit(reference, direct, &input);
    let mut truncated = input.clone();
    truncated.pop();
    assert!(reference.try_advance_bytes(&truncated).is_err());
    visit(reference, direct, &truncated);
    // A successful real protocol transcript with distinct calibration/sample
    // bytes compares each action and final capture, not only isolated cases.
    let mut state = reference.initial();
    let mut saw_capture = false;
    let mut now = 1000u64;
    for _ in 0..64 {
        let action = reference.action(&state);
        assert_eq!(action, direct.action(&state));
        let event = match tag(&action) {
            "transact" => {
                let request = payload(&action);
                let register = bytes(field(request, "write"))[0];
                let data = if byte(field(request, "read")) == 0 {
                    vec![]
                } else {
                    vec![match register {
                        208 => 96,
                        243 => 0,
                        _ => register.wrapping_mul(3),
                    }]
                };
                reference.bus("completed", &data, now)
            }
            "waiting" => {
                let StructuredInfoValueShape::Leaf(value) = payload(&action).shape() else {
                    panic!("deadline")
                };
                now = u64::from_le_bytes(value.try_into().unwrap());
                reference.tick(now)
            }
            "captured" => {
                assert_eq!(byte(field(&state, "phase")), 14);
                saw_capture = true;
                break;
            }
            other => panic!("unexpected transcript action: {other}"),
        };
        // Use monotonic event times even after a wait advances beyond1000.
        let input = reference.input_bytes(state.clone(), event);
        visit(reference, direct, &input);
        state = StructuredInfoValue::from_canonical_bytes(reference.advance_bytes(&input)).unwrap();
    }
    assert!(saw_capture);
    count
}

#[test]
fn revised_transition_preserves_native_front_and_ordered_failure_semantics() {
    let original = checked(ORACLE);
    let revised = checked(PRODUCTION);
    let front = |document: &conduit_plot::CheckedSyntaxDocument| {
        document
            .plots
            .iter()
            .find(|plot| plot.name == "bme280-protocol-transition")
            .unwrap()
            .runtime_front
            .clone()
    };
    assert_eq!(front(&original), front(&revised));
    assert_ne!(original.source_document_id, revised.source_document_id);
    for native in &revised.native_types {
        assert_eq!(
            original
                .native_types
                .iter()
                .find(|ty| ty.name == native.name),
            Some(native)
        );
    }
    assert_eq!(transition_programs(&original).len(), 12);
    assert_eq!(transition_programs(&revised).len(), 2);
    let mut reference = Fixture::staged_oracle();
    let mut revised = Fixture::new();
    let initial = reference.initial();
    for phase in [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 255,
    ] {
        for address in [117, 118] {
            let state = replace(
                &reference.with_phase(&initial, phase),
                "address",
                &[address],
            );
            for event in [
                reference.tick(123),
                reference.bus("completed", &[0], 123),
                reference.clock_failure("provider-lost"),
            ] {
                let input = reference.input_bytes(state.clone(), event);
                compare(&mut reference, &mut revised, &input);
            }
        }
    }
    for (phase, index) in [(7, 23), (7, 24), (9, 6), (9, 7), (12, 7), (13, 7), (13, 8)] {
        let state = reference.with_phase(&initial, phase);
        let state = reference.with_index(&state, index);
        let state = replace(&state, "polls", &[7]);
        for event in [
            reference.bus("completed", &[0x55], 123),
            reference.bus("short", &[], 123),
        ] {
            let input = reference.input_bytes(state.clone(), event);
            compare(&mut reference, &mut revised, &input);
        }
    }
}

#[test]
fn revised_transition_evaluates_changing_inputs_without_execution_allocations() {
    let mut fixture = Fixture::new();
    let initial = fixture.initial();
    let inputs: Vec<_> = (0..16)
        .map(|phase| {
            let state = fixture.with_phase(&initial, phase);
            fixture.input_bytes(state, fixture.bus("completed", &[0], 123))
        })
        .collect();
    for input in &inputs {
        fixture.try_advance_bytes(input).unwrap();
    }
    let (_, costs) = allocation_probe::observe(|| {
        for input in &inputs {
            fixture.try_advance_bytes(input).unwrap();
        }
    });
    assert_eq!(costs.allocations, 0);
    assert_eq!(costs.reallocations, 0);
}
