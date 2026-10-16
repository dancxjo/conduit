//! Exact owned filter equation through production local-expression kernel factories.
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
#[path = "common_acoustic_reference/parent_kernel.rs"]
mod flow_kernel;
use flow_kernel::allocation_probe;
#[path = "common_acoustic_value/kernel.rs"]
mod kernel;
#[path = "common_acoustic_reference/mod.rs"]
mod preparation;
const OWNER: &str = include_str!("../../../semantics/audio/dsp/one-pole.conduit");
fn source() -> String {
    let (_, definition) = OWNER.split_once("plot dsp/one-pole (").unwrap();
    let value = format!("plot dsp/one-pole-value ({definition}").replace("...|", "");
    format!("{OWNER}\n{value}\nplot one-pole-proof (\n >> value: OnePoleFrame\n result: OnePoleOutcome >>\n) {{\n filter: dsp/one-pole-value()\n value >> filter.frame\n filter.result >> result\n}}\n")
}
fn program(source: &str, entry: &str) -> PortableExpressionProgram {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn input(
    p: &PortableExpressionProgram,
    x: i32,
    history: i32,
    a: i32,
    n: i32,
    reset: bool,
) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
        panic!()
    };
    StructuredInfoValue::record(
        p.input_type.clone(),
        fields
            .iter()
            .map(|f| {
                let bytes = match f.name() {
                    "reset" => vec![u8::from(reset)],
                    "cancelled" => vec![0],
                    name => match name {
                        "sample" => x,
                        "history" => history,
                        "retention_q14" => a,
                        "remaining" => n,
                        _ => panic!(),
                    }
                    .to_le_bytes()
                    .to_vec(),
                };
                StructuredFieldValue::new(
                    f.name(),
                    StructuredInfoValue::leaf(f.value_type().clone(), bytes).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn history(bytes: &[u8]) -> (i32, i32, i32, i32) {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!()
    };
    assert_eq!(tag, "samples");
    let field = |name| {
        let value = preparation::field(payload, name);
        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            panic!()
        };
        i32::from_le_bytes(bytes.try_into().unwrap())
    };
    (
        field("low"),
        field("high"),
        field("history"),
        field("remaining"),
    )
}
fn run(blocks: &[usize]) -> Vec<Vec<u8>> {
    let source = source();
    let p = program(&source, "dsp/one-pole-value");
    assert_eq!(
        program(OWNER, "dsp/one-pole").canonical_hex().unwrap(),
        p.canonical_hex().unwrap()
    );
    let mut outputs = Vec::new();
    let mut committed: Option<Vec<u8>> = None;
    let mut index = 0;
    for &block in blocks {
        // Each window re-prepares its immutable Plan from Source and reloads committed state.
        let (plan, front, _) = preparation::prepare(&source, "OnePoleFrame", "one-pole-proof");
        // Preparation checks the full Source/checked/expanded identities, including
        // its explicit Audio declaration context and this reviewed wrapper.
        let (mut state, mut remaining) = committed
            .as_ref()
            .map(|bytes| {
                let (_, _, y, n) = history(bytes);
                (y, n)
            })
            .unwrap_or((0, 16));
        let contract = CheckedValueContract::new(
            p.input_type.profile().unwrap().value_kind().clone(),
            32768,
            vec![],
        )
        .unwrap();
        for _ in 0..block {
            let x = if index % 2 == 0 { 32767 } else { -32767 };
            let a = [0, 8192, 16383, 1][index % 4];
            let reset = index == 0 || index == 9;
            let bytes = input(&p, x, state, a, remaining, reset);
            let pair = conduit_std_host::flow_activation::PreparedValuePairFactory::new(
                &contract,
                &p.input_type,
                &contract,
                &p.input_type,
            )
            .unwrap();
            let output =
                kernel::run(plan.clone(), &front, pair, vec![], bytes.clone(), false).unwrap();
            assert_eq!(output, p.evaluate(&bytes).unwrap());
            let (low, high, next, left) = history(&output);
            let prior = if reset { 0 } else { state };
            let wide =
                (i64::from(x) * (16384 - i64::from(a)) + i64::from(prior) * i64::from(a)) / 16384;
            assert_eq!(
                (low, high, next, left),
                (
                    wide as i32,
                    (i64::from(x) - wide).clamp(-32767, 32767) as i32,
                    wide as i32,
                    15 - index as i32
                )
            );
            state = next;
            remaining = left;
            committed = Some(output.clone());
            outputs.push(output);
            index += 1;
        }
    }
    assert_eq!(index, 16);
    outputs
}
#[test]
fn installed_owned_equation_preserves_state_across_prepared_windows_pressure_and_close() {
    assert_eq!(run(&[16]), run(&[1, 5, 10]));
}
#[test]
fn cancellation_of_held_pure_output_does_not_commit_a_new_cursor() {
    let source = source();
    let p = program(&source, "dsp/one-pole-value");
    let bytes = input(&p, 32767, -32767, 8192, 16, false);
    let original = bytes.clone();
    let (plan, front, pair) = preparation::prepare(&source, "OnePoleFrame", "one-pole-proof");
    assert!(kernel::run(plan, &front, pair, vec![], bytes.clone(), true).is_none());
    assert_eq!(bytes, original);
    let (plan, front, pair) = preparation::prepare(&source, "OnePoleFrame", "one-pole-proof");
    assert_eq!(
        kernel::run(plan, &front, pair, vec![], original.clone(), false).unwrap(),
        p.evaluate(&original).unwrap()
    );
}

#[test]
fn owned_closing_flow_preserves_all_frames_under_input_pressure_and_held_outputs() {
    let p = program(OWNER, "dsp/one-pole");
    let inputs = (0..8)
        .map(|i| {
            input(
                &p,
                (i - 4) * 4096,
                (4 - i) * 2048,
                [0, 8192, 16383, 1][i as usize % 4],
                8 - i,
                i == 0,
            )
        })
        .collect::<Vec<_>>();
    let expected = inputs
        .iter()
        .map(|bytes| p.evaluate(bytes).unwrap())
        .collect::<Vec<_>>();
    for (gap, hold) in [(0, 1), (3, 5)] {
        let (plan, front, _) = preparation::prepare(OWNER, "OnePoleFrame", "dsp/one-pole");
        assert_eq!(
            front.inputs()[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(
            front.outputs()[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        let registry = conduit_std_host::flow_activation::standard_child_registry().unwrap();
        let outputs = flow_kernel::run(plan, &front, registry, &inputs, gap, hold);
        assert_eq!(outputs, expected);
    }
}
