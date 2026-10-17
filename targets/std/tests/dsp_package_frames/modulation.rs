//! Abrupt coefficient changes stay within the explicit experimental profile.
use super::*;
fn render(blocks: &[usize]) -> Vec<Vec<u8>> {
    let package = conduit_semantic_catalog::prepare_dsp_source_package().unwrap();
    let stage = &package
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "SharedExcitationStage")
        .unwrap()
        .value_type;
    let contract =
        CheckedValueContract::new(stage.profile().unwrap().value_kind().clone(), 32768, vec![])
            .unwrap();
    let mut control = payload(
        &StructuredInfoValue::from_canonical_bytes(&initial(31, 0, 0)).unwrap(),
        "context",
    );
    let schedule = [
        (8192_i64, 4096_i64),
        (-8192, -4096),
        (8192, -4096),
        (-8192, 4096),
        (0, 0),
        (1, -1),
    ];
    let (mut word, mut y1, mut y2) = (31_i64, 0_i64, 0_i64);
    let mut frame = 0;
    let mut outputs = vec![];
    for &block in blocks {
        let (plan, front, _) = prepare("dsp/experimental-q14-control-frame");
        for _ in 0..block {
            let (b, c) = schedule[frame];
            control = independent::cursor_field(&control, "b", (b as i32).to_le_bytes().to_vec());
            control = independent::cursor_field(&control, "c", (c as i32).to_le_bytes().to_vec());
            let pair = PreparedValuePairFactory::new(&contract, stage, &contract, stage).unwrap();
            let output = kernel::run(
                plan.clone(),
                &front,
                pair,
                vec![],
                control.canonical_bytes().unwrap(),
                false,
            )
            .unwrap();
            control = payload(
                &StructuredInfoValue::from_canonical_bytes(&output).unwrap(),
                "context",
            );
            let cursor = field(&control, "cursor");
            word = (word * 25173 + 13849) % 65536;
            let filtered =
                (((word - 32768) * 16384 + b * y1 + c * y2) / 16384).clamp(-32767, 32767);
            assert_eq!(
                integer(&field(&cursor, "sample")),
                (filtered * (frame as i64 * 32) / 256) as i32
            );
            assert_eq!(integer(&field(&cursor, "word")), word as i32);
            assert_eq!(integer(&field(&cursor, "y1")), filtered as i32);
            assert_eq!(integer(&field(&cursor, "y2")), y1 as i32);
            assert_eq!(integer(&field(&cursor, "b")), b as i32);
            assert_eq!(integer(&field(&cursor, "c")), c as i32);
            assert_eq!(integer(&field(&cursor, "remaining")), 159 - frame as i32);
            assert!(filtered.abs() <= 32767);
            y2 = y1;
            y1 = filtered;
            frame += 1;
            outputs.push(output);
        }
        control =
            StructuredInfoValue::from_canonical_bytes(&control.canonical_bytes().unwrap()).unwrap();
    }
    assert_eq!(frame, schedule.len());
    outputs
}
#[test]
fn abrupt_experimental_coefficient_changes_match_wide_reference_and_fresh_resume() {
    assert_eq!(render(&[6]), render(&[1, 2, 3]));
}
