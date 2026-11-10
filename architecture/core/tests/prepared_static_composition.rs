//! CPU allocation proof for immutable-schema preparation and ordinary encoding.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;
fn linked(ty: &StructuredInfoType) -> &'static [u8] {
    // Fixture fabrication only: production Seeds emit these bytes into flash.
    Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice())
}
#[test]
fn immutable_schema_preserves_canonical_outputs_and_funds_all_allocations() {
    let leaf = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let record = StructuredInfoType::record(
        kind_id("test/static-record"),
        vec![StructuredFieldType::new("sample", leaf.clone()).unwrap()],
    )
    .unwrap();
    let schema = linked(&record);
    let (composer, cost) =
        probe::observe(|| PreparedStructuredComposer::try_from_static_schema(schema, 256, 4096));
    let mut composer = composer.unwrap();
    let bytes = composer.prepared_heap_bytes();
    assert_eq!(cost.requested_bytes, bytes);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(cost.allocations, 2); // spans and mutable output; schema borrowed
    assert!(PreparedStructuredComposer::try_from_static_schema(schema, 256, bytes).is_ok());
    let (short, refused) = probe::observe(|| {
        PreparedStructuredComposer::try_from_static_schema(schema, 256, bytes - 1)
    });
    assert!(matches!(
        short,
        Err(StructuredCompositionPreparationRefusal::Capacity)
    ));
    assert_eq!((refused.allocations, refused.reallocations), (0, 0));
    for nth in 1..=cost.allocations {
        let (result, failed) = probe::observe_refusing(nth, || {
            PreparedStructuredComposer::try_from_static_schema(schema, 256, bytes)
        });
        assert!(matches!(
            result,
            Err(StructuredCompositionPreparationRefusal::Allocation)
        ));
        assert_eq!(failed.refused_allocations, 1);
        assert_eq!(failed.allocations, failed.deallocations);
    }
    let child = StructuredInfoValue::leaf(leaf, vec![9]).unwrap();
    let child_bytes = child.canonical_bytes().unwrap();
    let expected = StructuredInfoValue::record(
        record,
        vec![StructuredFieldValue::new("sample", child).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let ((), runtime) = probe::observe(|| {
        let value = validate_canonical_structured_value(&child_bytes).unwrap();
        assert_eq!(composer.record(&[value]).unwrap(), expected);
        assert_eq!(
            composer.record(&[]),
            Err(StructuredInfoRefusal::WrongRecordFields)
        );
        assert_eq!(composer.encoded(), expected);
    });
    assert_eq!((runtime.allocations, runtime.reallocations), (0, 0));
}
#[test]
fn leaf_variant_and_bounded_sequence_reuse_the_same_runtime_composer() {
    let leaf = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let variant = StructuredInfoType::variant(
        kind_id("test/static-variant"),
        vec![StructuredVariantCase::new("ready", leaf.clone()).unwrap()],
    )
    .unwrap();
    let sequence = StructuredInfoType::bounded_sequence(leaf.clone(), 1, 2).unwrap();
    let value = StructuredInfoValue::leaf(leaf.clone(), vec![7]).unwrap();
    let encoded = value.canonical_bytes().unwrap();
    let view = validate_canonical_structured_value(&encoded).unwrap();
    let mut a =
        PreparedStructuredComposer::try_from_static_schema(linked(&leaf), 128, 1024).unwrap();
    assert_eq!(a.leaf(&[7]).unwrap(), encoded);
    let mut b =
        PreparedStructuredComposer::try_from_static_schema(linked(&variant), 256, 1024).unwrap();
    assert_eq!(
        b.variant("ready", view).unwrap(),
        StructuredInfoValue::variant(variant, "ready", value.clone())
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    let mut c =
        PreparedStructuredComposer::try_from_static_schema(linked(&sequence), 256, 1024).unwrap();
    assert_eq!(
        c.collection(&[view]).unwrap(),
        StructuredInfoValue::sequence(sequence, vec![value])
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    assert_eq!(
        c.collection(&[]),
        Err(StructuredInfoRefusal::WrongCollectionLength)
    );
    let (result, cost) =
        probe::observe(|| PreparedStructuredComposer::try_from_static_schema(&[255], 128, 1024));
    assert!(matches!(
        result,
        Err(StructuredCompositionPreparationRefusal::Canonical(_))
    ));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
}

#[test]
fn primitive_collection_replay_cannot_grow_prepared_storage() {
    let leaf = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let seq = StructuredInfoType::sequence(leaf, 2).unwrap();
    let mut composer =
        PreparedStructuredComposer::try_from_static_schema(linked(&seq), 128, 1024).unwrap();
    let ((), cost) = probe::observe(|| {
        let bytes = [7u8, 9];
        let result = composer
            .leaf_collection(bytes.iter().map(core::slice::from_ref))
            .unwrap();
        validate_canonical_structured_value(result).unwrap();
        struct Changing {
            remaining: usize,
        }
        impl Clone for Changing {
            fn clone(&self) -> Self {
                Self { remaining: 1 }
            }
        }
        impl Iterator for Changing {
            type Item = &'static [u8];
            fn next(&mut self) -> Option<Self::Item> {
                if self.remaining == 0 {
                    None
                } else {
                    self.remaining -= 1;
                    Some(&[7])
                }
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (1, Some(1))
            }
        }
        impl ExactSizeIterator for Changing {
            fn len(&self) -> usize {
                1
            }
        }
        assert_eq!(
            composer.leaf_collection(Changing { remaining: 2 }),
            Err(StructuredInfoRefusal::WrongCollectionLength)
        );
        assert!(composer.encoded().is_empty());
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
}
