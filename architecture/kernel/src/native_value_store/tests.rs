use super::*;
use crate::FixedValueStore;
#[test]
fn native_result_never_enters_canonical_storage_and_cannot_be_read_as_bytes() {
    let mut store =
        NativeU64ValueStore::<_, 2>::new(FixedValueStore::<2, 8>::new(16).unwrap(), 16).unwrap();
    let canonical = store.store(b"boundary").unwrap();
    let native = store.store_native_u64(u64::MAX).unwrap();
    assert_eq!(store.canonical().used_items(), 1);
    assert_eq!(store.canonical().used_bytes(), 8);
    assert_eq!(store.native_u64(native), Ok(Some(u64::MAX)));
    assert_eq!(store.native_u64(canonical), Ok(None));
    assert_eq!(store.get(native), Err(StorageError::RepresentationMismatch));
    assert_eq!(store.get(canonical), Ok(b"boundary".as_slice()));
    assert_eq!(store.used_items(), 2);
    assert_eq!(store.used_bytes(), 16);
    store.retain(native).unwrap();
    store.release(native).unwrap();
    assert_eq!(store.reference_count(native), Ok(1));
    store.release(native).unwrap();
    assert_eq!(store.native_u64(native), Err(StorageError::StaleReference));
    let next = store.store_native_u64(17).unwrap();
    assert_eq!(next.slot, native.slot);
    assert_ne!(next.generation, native.generation);
    assert_eq!(store.native_u64(native), Err(StorageError::StaleReference));
}
#[test]
fn pressure_and_generation_exhaustion_refuse_without_publishing_values() {
    let mut store =
        NativeU64ValueStore::<_, 2>::new(FixedValueStore::<1, 8>::new(8).unwrap(), 8).unwrap();
    let value = store.store_native_u64(1).unwrap();
    assert_eq!(
        store.store_native_u64(2),
        Err(StorageError::ByteCapacityExceeded)
    );
    assert_eq!(store.native_u64(value), Ok(Some(1)));
    store.release(value).unwrap();
    store
        .native
        .iter_mut()
        .for_each(|s| s.generation = u16::MAX);
    assert_eq!(
        store.store_native_u64(2),
        Err(StorageError::GenerationExhausted)
    );
    assert_eq!(store.used_items(), 0);
    store.clear();
    assert_eq!(
        store.store_native_u64(2),
        Err(StorageError::GenerationExhausted)
    );
}
#[test]
fn unsupported_stores_refuse_and_native_reference_overflow_keeps_ownership() {
    let mut canonical = FixedValueStore::<1, 8>::new(8).unwrap();
    assert_eq!(
        canonical.store_native_u64(1),
        Err(StorageError::RepresentationMismatch)
    );
    let mut store = NativeU64ValueStore::<_, 1>::new(canonical, 8).unwrap();
    let value = store.store_native_u64(9).unwrap();
    store.native[0].references = u16::MAX;
    assert_eq!(store.retain(value), Err(StorageError::ReferenceOverflow));
    assert_eq!(store.reference_count(value), Ok(u16::MAX));
}
