use super::*;
#[test]
fn checkpoint_extent_is_exact_readonly_and_refuses_unknown_or_stale_custody() {
    let (mut store, mut owner, _, key) = park_tests::fixture();
    assert!(checkpoint_extent_store(&store).is_err());
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let expected = (
        store.journal.generation().unwrap(),
        store
            .journal
            .payload_length(&store.binding)
            .unwrap()
            .unwrap(),
    );
    assert_eq!(checkpoint_extent_store(&store).unwrap(), expected);
    assert_eq!(checkpoint_extent_store(&store).unwrap(), expected);
    assert_eq!(store.journal.generation(), Some(expected.0));
    store.ready = false;
    assert!(checkpoint_extent_store(&store).is_err());
    store.ready = true;
    store.parked = true;
    assert!(checkpoint_extent_store(&store).is_err());
    store.parked = false;
    store.failed = true;
    assert!(checkpoint_extent_store(&store).is_err());
    store.failed = false;
    store.binding[0] ^= 1;
    assert!(checkpoint_extent_store(&store).is_err());
    store.binding[0] ^= 1;
    store
        .journal
        .save(&store.binding, &[7; 32], b"changed-generation")
        .unwrap();
    assert!(checkpoint_extent_store(&store).is_err());
}
