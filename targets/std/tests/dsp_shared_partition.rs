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
