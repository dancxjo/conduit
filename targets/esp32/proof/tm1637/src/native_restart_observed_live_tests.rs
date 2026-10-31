use super::*;
#[test]
fn cached_live_layout_requires_exact_durable_ready_source_without_cold_relaxation() {
    let (mut store, mut owner, _, key) = super::super::park_tests::fixture();
    let owner = owner.as_mut().unwrap();
    checkpoint_store(&mut store, owner, &key).unwrap();
    assert!(live_custody(&store, owner).is_ok());
    // Cache content must come from real authenticated/readback layout persistence.
    store.evidence = Some(owner.evidence().clone());
    store.ready = false;
    let r = super::tests::receipt(&owner.evidence().body_id, &[7; 32], 3);
    let checked = persist(&mut store, r.clone()).unwrap();
    store.ready = true;
    assert_eq!(
        live_custody(&store, owner)
            .unwrap()
            .layouts
            .find(&[3; 32])
            .unwrap()
            .bytes(),
        checked.bytes()
    );
    assert!(live_custody(&store, owner)
        .unwrap()
        .layouts
        .find(&[4; 32])
        .is_none());
    let generation = store.journal.generation();
    store.parked = true;
    assert!(live_custody(&store, owner).is_err());
    store.parked = false;
    store.failed = true;
    assert!(live_custody(&store, owner).is_err());
    store.failed = false;
    store.ready = false;
    assert!(live_custody(&store, owner).is_err());
    store.ready = true;
    store.binding[0] ^= 1;
    assert!(live_custody(&store, owner).is_err());
    store.binding[0] ^= 1;
    assert_eq!(store.journal.generation(), generation);
    // A valid alternate-bank write invalidates the former cached source token.
    store
        .journal
        .save(&store.binding, &[7; 32], b"changed-generation")
        .unwrap();
    assert!(live_custody(&store, owner).is_err());
}
