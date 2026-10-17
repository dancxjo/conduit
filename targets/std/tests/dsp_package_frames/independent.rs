//! Separate periodic, aspiration and frication cursors through the shared graph.
use super::*;
fn render(aspiration_seed: i32) -> Vec<StructuredInfoValue> {
    let package = conduit_semantic_catalog::prepare_dsp_source_package().unwrap();
    let input_type = &package
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "IndependentExcitationInput")
        .unwrap()
        .value_type;
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
    let seeds = [
        music_initial(31),
        initial(aspiration_seed, 4096, -2048),
        initial(97, -4096, 1024),
    ];
    let mut controls = seeds
        .iter()
        .map(|bytes| {
            payload(
                &StructuredInfoValue::from_canonical_bytes(bytes).unwrap(),
                "context",
            )
        })
        .collect::<Vec<_>>();
    let (plan, front, _) = prepare("dsp/experimental-independent-excitation");
    let mut words = [i64::from(aspiration_seed), 97];
    let mut history = [[0_i64; 2]; 2];
    let mut outputs = vec![];
    let cubic = |x: i64| {
        if x <= 0 || x >= 256 {
            0
        } else {
            x * x * (256 - x) / 256
        }
    };
    for frame in 0..3_i64 {
        let input = StructuredInfoValue::record(
            input_type.clone(),
            ["voiced", "aspiration", "frication"]
                .into_iter()
                .zip(&controls)
                .map(|(name, value)| StructuredFieldValue::new(name, value.clone()).unwrap())
                .collect(),
        )
        .unwrap();
        let pair = PreparedValuePairFactory::new(&contract, stage, &contract, stage).unwrap();
        let bytes = kernel::run(
            plan.clone(),
            &front,
            pair,
            vec![],
            input.canonical_bytes().unwrap(),
            false,
        )
        .unwrap();
        let result = payload(
            &StructuredInfoValue::from_canonical_bytes(&bytes).unwrap(),
            "excitation",
        );
        let voices = field(&result, "voices");
        let mut pcm = [0_i64; 2];
        for (i, (b, c)) in [(4096, -2048), (-4096, 1024)].into_iter().enumerate() {
            words[i] = (words[i] * 25173 + 13849) % 65536;
            let filtered = (((words[i] - 32768) * 16384 + b * history[i][0] + c * history[i][1])
                / 16384)
                .clamp(-32767, 32767);
            history[i] = [filtered, history[i][0]];
            pcm[i] = filtered * (frame * 32) / 256;
            let cursor = field(
                &field(&voices, if i == 0 { "aspiration" } else { "frication" }),
                "cursor",
            );
            assert_eq!(integer(&field(&cursor, "word")), words[i] as i32);
            assert_eq!(integer(&field(&cursor, "y1")), filtered as i32);
        }
        let phase = frame * 256;
        let previous = if phase < 256 {
            0
        } else {
            (phase - 256) * 256 / 3072
        };
        let voiced = (cubic(phase * 256 / 3072) - cubic(previous)) * (frame * 32) / 256;
        let noise_mix = pcm[0] * 128 / 256 + pcm[1] * 128 / 256;
        let expected = voiced * 128 / 256 + noise_mix * 128 / 256;
        assert_eq!(integer(&field(&result, "sample")), expected as i32);
        controls = ["voiced", "aspiration", "frication"]
            .into_iter()
            .map(|name| field(&voices, name))
            .collect();
        outputs.push(voices);
    }
    outputs
}
#[test]
fn changing_only_aspiration_seed_does_not_change_voiced_or_frication_state() {
    let original = render(13);
    let changed = render(14);
    for (original, changed) in original.iter().zip(&changed) {
        assert_eq!(field(original, "voiced"), field(changed, "voiced"));
        assert_eq!(field(original, "frication"), field(changed, "frication"));
        assert_ne!(field(original, "aspiration"), field(changed, "aspiration"));
    }
}
pub(super) fn cursor_field(
    control: &StructuredInfoValue,
    name: &str,
    bytes: Vec<u8>,
) -> StructuredInfoValue {
    let cursor = field(control, "cursor");
    let old = field(&cursor, name);
    let replacement = StructuredInfoValue::leaf(old.value_type().clone(), bytes).unwrap();
    let StructuredInfoValueShape::Record(cursor_fields) = cursor.shape() else {
        panic!("cursor")
    };
    let cursor = StructuredInfoValue::record(
        cursor.value_type().clone(),
        cursor_fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    if f.name() == name {
                        replacement.clone()
                    } else {
                        f.value().clone()
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let StructuredInfoValueShape::Record(fields) = control.shape() else {
        panic!("control")
    };
    StructuredInfoValue::record(
        control.value_type().clone(),
        fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    if f.name() == "cursor" {
                        cursor.clone()
                    } else {
                        f.value().clone()
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn invalid_roles_exhaustion_seed_coefficients_and_timebase_reject_the_whole_frame() {
    let package = conduit_semantic_catalog::prepare_dsp_source_package().unwrap();
    let input_type = &package
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "IndependentExcitationInput")
        .unwrap()
        .value_type;
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
    let seeds = [
        music_initial(31),
        initial(13, 4096, -2048),
        initial(97, -4096, 1024),
    ];
    let original = seeds
        .iter()
        .map(|bytes| {
            payload(
                &StructuredInfoValue::from_canonical_bytes(bytes).unwrap(),
                "context",
            )
        })
        .collect::<Vec<_>>();
    let (plan, front, _) = prepare("dsp/experimental-independent-excitation");
    for case in ["role", "exhausted", "seed", "coefficient", "timebase"] {
        let mut controls = original.clone();
        match case {
            "role" => {
                controls[0] = payload(
                    &StructuredInfoValue::from_canonical_bytes(&initial(31, 0, 0)).unwrap(),
                    "context",
                )
            }
            "exhausted" => {
                controls[1] = cursor_field(&controls[1], "remaining", 0_u32.to_le_bytes().to_vec())
            }
            "seed" => {
                controls[1] = cursor_field(&controls[1], "word", (-1_i32).to_le_bytes().to_vec())
            }
            "coefficient" => {
                controls[1] = cursor_field(&controls[1], "b", 8193_i32.to_le_bytes().to_vec())
            }
            "timebase" => {
                controls[1] = cursor_field(&controls[1], "frame", 1_i32.to_le_bytes().to_vec())
            }
            _ => unreachable!(),
        }
        let input = StructuredInfoValue::record(
            input_type.clone(),
            ["voiced", "aspiration", "frication"]
                .into_iter()
                .zip(&controls)
                .map(|(name, value)| StructuredFieldValue::new(name, value.clone()).unwrap())
                .collect(),
        )
        .unwrap();
        let pair = PreparedValuePairFactory::new(&contract, stage, &contract, stage).unwrap();
        let bytes = kernel::run(
            plan.clone(),
            &front,
            pair,
            vec![],
            input.canonical_bytes().unwrap(),
            false,
        )
        .unwrap();
        let result = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        let StructuredInfoValueShape::Variant { tag, .. } = result.shape() else {
            panic!("outcome")
        };
        assert_eq!(tag, "rejected", "{case}");
    }
}
