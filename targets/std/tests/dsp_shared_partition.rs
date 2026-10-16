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
                let prepared = planning::prepare(&initial, &tick);
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
