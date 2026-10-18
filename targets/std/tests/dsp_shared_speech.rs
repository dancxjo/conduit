//! Admitted IPA selects a Source profile before the shared finite DSP Flow.
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
#[path = "common_acoustic_value/kernel.rs"]
mod kernel;
#[path = "common_shared_dsp/parent_plan.rs"]
mod planning;
#[path = "common_acoustic_reference/mod.rs"]
mod preparation;
#[path = "common_acoustic_timed/partition_kernel.rs"]
mod producer;
use producer::allocation_probe;
#[path = "common_shared_dsp/source.rs"]
mod source;
use preparation::field;

fn admitted_phone(spelling: &str) -> Vec<u8> {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_speech::authoring::install(&mut startup).unwrap();
    conduit_speech::ipa_constructors::install(&mut startup, &mut profiles).unwrap();
    let source =
        include_str!("../../../semantics/speech/examples/ipa/quoted-transcriptions.conduit")
            .replace("speech/phonetic-from-ipa", "speech/phone-from-ipa")
            .replace("SpeechPhoneticTranscription", "SpeechPhoneNotation")
            .replace("ˈt͡ʃãː.n̩", spelling);
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    conduit_speech::ipa_constructors::validate_source(&syntax, &checked).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, &syntax.plots[0].name.text, &profiles)
            .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let prepared = conduit_speech::ipa_constructors::prepare_configuration(
        conduit_speech::ipa_constructors::IpaConstructor::Phone,
        &expanded.expanded.gears[0].configuration,
    )
    .unwrap();
    prepared.bytes().to_vec()
}

#[test]
fn admitted_ipa_drives_shared_frication_and_unsupported_phone_refuses() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let fixture = format!(
                "{}\n{}",
                source::source(),
                include_str!("fixtures/shared-speech-frication-profile.conduit")
            );
            for (phone, coefficient) in [("s", Some(-8192_i32)), ("ʃ", Some(-4096)), ("a", None)] {
                let mut startup = StartupCatalog::new();
                // The reference entrance checks the complete Audio owner Source.
                // Import only its required Speech root, preserving every checked
                // refinement/law without redeclaring the imported Audio names.
                let phone_type = conduit_speech::authoring::checked_types()
                    .unwrap()
                    .into_iter()
                    .find(|ty| ty.name == "SpeechPhoneNotation")
                    .unwrap();
                startup
                    .insert_checked_native_type("SpeechPhoneNotation", &phone_type)
                    .unwrap();
                let (plan, front, pair) = preparation::prepare_with_catalogues(
                    &fixture,
                    "SharedExcitationStage",
                    "shared-speech-frication-profile",
                    startup,
                    ProfileCatalog::new(),
                );
                let input = admitted_phone(phone);
                println!(
                    "ipa={phone} input-digest={:?} source={} plan={}",
                    semantic_digest("fixture/shared-ipa-input", &input),
                    plan.source_document_id.as_str(),
                    plan.plan_id.as_str()
                );
                let initial = kernel::run(plan, &front, pair, vec![], input, false).unwrap();
                let value = StructuredInfoValue::from_canonical_bytes(&initial).unwrap();
                let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
                    panic!("profile")
                };
                let Some(coefficient) = coefficient else {
                    assert_eq!(tag, "rejected");
                    continue;
                };
                assert_eq!(tag, "context");
                let b = field(&field(payload, "cursor"), "b");
                let StructuredInfoValueShape::Leaf(bytes) = b.shape() else {
                    panic!("coefficient")
                };
                assert_eq!(i32::from_le_bytes(bytes.try_into().unwrap()), coefficient);
                let (dsp_plan, dsp_front, dsp_pair) = preparation::prepare(
                    &source::source(),
                    "SharedExcitationStage",
                    "shared-excitation-frame",
                );
                let initial =
                    kernel::run(dsp_plan, &dsp_front, dsp_pair, vec![], initial, false).unwrap();
                // Explicit finite boundary: the actual committed profile is the
                // next Plan's initial accumulator, not a Rust-selected replacement.
                let tick_source = source::source();
                let checked = check_syntax_document(
                    &parse_syntax_document(
                        &tick_source
                            .split("plot shared-excitation-unpack (")
                            .next()
                            .unwrap()
                            .lines()
                            .filter(|l| !l.starts_with("with "))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    &StartupCatalog::new(),
                )
                .unwrap();
                let tick = expand_canonical_plot_for_authoring(
                    &checked,
                    "shared-excitation-tick",
                    &ProfileCatalog::new(),
                )
                .unwrap();
                let ConfigurationValue::Text(program) =
                    &tick.expanded.gears[0].configuration[0].value
                else {
                    panic!("tick")
                };
                let tick = conduit_plot::PortableExpressionProgram::from_canonical_hex(program)
                    .unwrap()
                    .evaluate(&[0])
                    .unwrap();
                let mut reference = None;
                for windows in [&[64, 64, 31][..], &[31, 64, 64][..]] {
                    let mut accumulator = initial.clone();
                    let mut all = vec![initial.clone()];
                    for count in windows {
                        // Tick is an admitted Boolean variant; no acoustic policy.
                        let prepared = planning::prepare(&accumulator, &tick, *count);
                        let blocks = if windows[0] == 64 {
                            vec![*count]
                        } else {
                            vec![1, *count - 1]
                        };
                        let states = producer::run(
                            prepared.plan,
                            &prepared.front,
                            &prepared.registry,
                            &prepared.items,
                            &blocks,
                            producer::Delivery {
                                gap: 7,
                                hold: 4,
                                signs: conduit_composite::KernelCompositeSignStorage {
                                    additional_local_items: 8192,
                                    additional_remote_items: 4096,
                                },
                            },
                        );
                        accumulator = states.last().unwrap().clone();
                        all.extend(states);
                    }
                    assert_eq!(all.len(), 160);
                    let (mut word, mut y1, mut y2) = (7_u64, 0_i32, 0_i32);
                    for (frame, bytes) in all.iter().enumerate() {
                        let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
                        let StructuredInfoValueShape::Variant { payload, .. } = value.shape()
                        else {
                            panic!("state")
                        };
                        let cursor = field(payload, "cursor");
                        word = (word * 25173 + 13849) & 65535;
                        let level = if frame < 8 {
                            frame as i32 * 32
                        } else if frame > 144 {
                            (160 - frame) as i32 * 16
                        } else {
                            256
                        };
                        let drive = ((word as i32 - 32768) * level / 256).clamp(-32767, 32767);
                        let sample = (drive
                            .checked_mul(16384)
                            .unwrap()
                            .checked_add(coefficient.checked_mul(y1).unwrap())
                            .unwrap()
                            .checked_add((-4096_i32).checked_mul(y2).unwrap())
                            .unwrap()
                            / 16384)
                            .clamp(-32767, 32767);
                        for (name, expected) in [
                            ("word", word as i32),
                            ("frame", frame as i32 + 1),
                            ("sample", sample),
                            ("y1", sample),
                            ("y2", y1),
                        ] {
                            let value = field(&cursor, name);
                            let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                                panic!("integer")
                            };
                            assert_eq!(i32::from_le_bytes(bytes.try_into().unwrap()), expected);
                        }
                        y2 = y1;
                        y1 = sample;
                    }

                    if let Some(previous) = &reference {
                        assert_eq!(&all, previous);
                    }
                    reference = Some(all);
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
