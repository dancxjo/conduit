//! One planned Flow retains actual excitation state across producer partitions.
use conduit_core::*;
#[path = "common_shared_dsp/parent_plan.rs"]
mod planning;
#[path = "common_acoustic_timed/partition_kernel.rs"]
mod producer;
#[path = "common_shared_dsp/seed.rs"]
mod seed;
#[path = "common_shared_dsp/source.rs"]
mod source;
fn field(value: &StructuredInfoValue, name: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
        .clone()
}
#[test]
fn actual_parent_state_and_seed_match_across_blocks_pressure_and_close() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = source::source();
            let initial = seed::seed(&source, 7);
            let tick = seed::tick(&source);
            let mut reference = None;
            let mut identity = None;
            for (partitions, gap, hold) in [(&[64][..], 0, 1), (&[1, 15, 17, 31][..], 7, 4)] {
                let prepared = planning::prepare(&initial, &tick, 64);
                if let Some(previous) = &identity {
                    assert_eq!(&prepared.plan.source_document_id, previous);
                }
                identity = Some(prepared.plan.source_document_id.clone());
                let states = producer::run(
                    prepared.plan,
                    &prepared.front,
                    &prepared.registry,
                    &prepared.items,
                    partitions,
                    producer::Delivery {
                        gap,
                        hold,
                        signs: conduit_composite::KernelCompositeSignStorage {
                            additional_local_items: 8192,
                            additional_remote_items: 4096,
                        },
                    },
                );
                let mut word = 7_u64;
                for (frame, bytes) in states.iter().enumerate() {
                    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
                    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
                        panic!("context")
                    };
                    assert_eq!(tag, "context");
                    let cursor = field(payload, "cursor");
                    word = (word * 25173 + 13849) & 65535;
                    let level = if frame < 8 { frame as i64 * 32 } else { 256 };
                    let sample = (((word as i64 - 32768) * level) / 256).clamp(-32767, 32767);
                    for (name, expected) in [
                        ("word", word as i32),
                        ("frame", frame as i32 + 1),
                        ("sample", sample as i32),
                    ] {
                        let value = field(&cursor, name);
                        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                            panic!("integer")
                        };
                        assert_eq!(i32::from_le_bytes(bytes.try_into().unwrap()), expected);
                    }
                }
                if let Some(previous) = &reference {
                    assert_eq!(&states, previous);
                }
                reference = Some(states);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn complete_attack_sustain_release_and_seed_survive_fresh_play_windows() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = source::source();
            let tick = seed::tick(&source);
            let mut reference = None;
            // Different preparation boundaries as well as producer block sizes.
            for windows in [&[64, 64, 32][..], &[32, 64, 64][..]] {
                let mut initial = seed::seed(&source, 65535);
                let mut complete = Vec::new();
                for count in windows {
                    let prepared = planning::prepare(&initial, &tick, *count);
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
                    assert_eq!(states.len(), *count);
                    // The committed state, including remaining work, is the
                    // next Play's admitted accumulator; nothing is reseeded.
                    initial = states.last().unwrap().clone();
                    complete.extend(states);
                }
                assert_eq!(complete.len(), 160);
                let mut word = 65535_u64;
                for (frame, encoded) in complete.iter().enumerate() {
                    let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
                    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
                        panic!("context")
                    };
                    assert_eq!(tag, "context");
                    let cursor = field(payload, "cursor");
                    word = (word * 25173 + 13849) & 65535;
                    let level = if frame < 8 {
                        frame as i64 * 32
                    } else if frame > 144 {
                        (160 - frame) as i64 * 16
                    } else {
                        256
                    };
                    let sample = (((word as i64 - 32768) * level) / 256).clamp(-32767, 32767);
                    for (name, expected) in [
                        ("word", word as i32),
                        ("frame", frame as i32 + 1),
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
                }
                if let Some(previous) = &reference {
                    assert_eq!(&complete, previous);
                }
                reference = Some(complete);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn shared_filter_matches_independent_ordered_i32_profile_and_refuses_bad_coefficients() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = source::source();
            let tick = seed::tick(&source);
            for (b, c, valid) in [
                (-8192, -4096, true),
                (8192, -4096, true),
                (i32::MAX, 0, false),
                (0, i32::MIN, false),
            ] {
                let initial = seed::profile(&source, 7, b, c);
                let prepared = planning::prepare(&initial, &tick, 64);
                let states = producer::run(
                    prepared.plan,
                    &prepared.front,
                    &prepared.registry,
                    &prepared.items,
                    &[1, 15, 17, 31],
                    producer::Delivery {
                        gap: 7,
                        hold: 4,
                        signs: conduit_composite::KernelCompositeSignStorage {
                            additional_local_items: 8192,
                            additional_remote_items: 4096,
                        },
                    },
                );
                let initial_value = StructuredInfoValue::from_canonical_bytes(&initial).unwrap();
                let StructuredInfoValueShape::Variant { payload, .. } = initial_value.shape()
                else {
                    panic!("initial context")
                };
                let initial_cursor = field(payload, "cursor");
                let (mut word, mut y1, mut y2) = (7_u64, 0_i32, 0_i32);
                for (frame, encoded) in states.iter().enumerate() {
                    let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
                    let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
                        panic!("context")
                    };
                    let cursor = field(payload, "cursor");
                    if !valid {
                        assert_eq!(cursor, initial_cursor);
                        continue;
                    }
                    word = (word * 25173 + 13849) & 65535;
                    let level = if frame < 8 { frame as i32 * 32 } else { 256 };
                    let drive = ((word as i32 - 32768) * level / 256).clamp(-32767, 32767);
                    let filtered = drive
                        .checked_mul(16384)
                        .unwrap()
                        .checked_add(b.checked_mul(y1).unwrap())
                        .unwrap()
                        .checked_add(c.checked_mul(y2).unwrap())
                        .unwrap()
                        / 16384;
                    let sample = filtered.clamp(-32767, 32767);
                    for (name, expected) in [("sample", sample), ("y1", sample), ("y2", y1)] {
                        let value = field(&cursor, name);
                        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                            panic!("integer")
                        };
                        assert_eq!(i32::from_le_bytes(bytes.try_into().unwrap()), expected);
                    }
                    y2 = y1;
                    y1 = sample;
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn musical_periodic_source_matches_cubic_reference_and_block_partitions() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = source::source();
            let initial = seed::music(&source, 7);
            let tick = seed::tick(&source);
            let mut reference = None;
            for blocks in [&[64][..], &[1, 15, 17, 31][..]] {
                let prepared = planning::prepare(&initial, &tick, 64);
                let states = producer::run(
                    prepared.plan,
                    &prepared.front,
                    &prepared.registry,
                    &prepared.items,
                    blocks,
                    producer::Delivery {
                        gap: 7,
                        hold: 4,
                        signs: conduit_composite::KernelCompositeSignStorage {
                            additional_local_items: 8192,
                            additional_remote_items: 4096,
                        },
                    },
                );
                let mut samples = Vec::new();
                for (frame, encoded) in states.iter().enumerate() {
                    let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
                    let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
                        panic!("context")
                    };
                    let cursor = field(payload, "cursor");
                    let value = field(&cursor, "sample");
                    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                        panic!("sample")
                    };
                    let actual = i32::from_le_bytes(bytes.try_into().unwrap());
                    let phase = (frame % 16) as i64 * 256;
                    let current = phase * 256 / 3072;
                    let previous = if phase < 256 {
                        0
                    } else {
                        (phase - 256) * 256 / 3072
                    };
                    let cubic = |x: i64| {
                        if x <= 0 || x >= 256 {
                            0
                        } else {
                            x * x * (256 - x) / 256
                        }
                    };
                    let level = if frame < 8 { frame as i64 * 32 } else { 256 };
                    let expected = (cubic(current) - cubic(previous)) * level / 256;
                    assert_eq!(i64::from(actual), expected);
                    samples.push(actual);
                }
                assert!(samples.iter().any(|x| *x > 0));
                assert!(samples.iter().any(|x| *x < 0));
                assert_eq!(&samples[16..32], &samples[32..48]);
                assert_eq!(&samples[32..48], &samples[48..64]);
                if let Some(previous) = &reference {
                    assert_eq!(&states, previous);
                }
                reference = Some(states);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
