use conduit_audio::authoring;
use conduit_plot::{NativeLawContext, NativeLawWitness, StartupCatalog};

#[test]
fn exact_owner_replay_and_transactional_missing_context() {
    let document = authoring::checked_document_with_law_custody().unwrap();
    let expected = authoring::checked_types().unwrap();
    assert_eq!(document.native_types, expected);
    let witness = document.native_law_witness("AudioExactTimeOffset").unwrap();
    let anchors = witness.family_anchors().unwrap();
    let replay = NativeLawWitness::verify_with_context(
        &witness.material().unwrap(),
        &witness.checked_type().value_type,
        &NativeLawContext { families: &anchors },
    )
    .unwrap();
    assert_eq!(replay.checked_type(), witness.checked_type());
    let mut catalog = StartupCatalog::new();
    let before = catalog.clone();
    assert!(
        authoring::install_law_custody_for(&mut catalog, &["AudioExactTimeOffset", "Absent"])
            .is_err()
    );
    assert_eq!(catalog, before);
    assert!(document.native_law_witness("AudioClockBinding").is_none());
    assert!(authoring::install_law_custody_for(&mut catalog, &["AudioClockBinding"]).is_err());
    assert_eq!(catalog, before);
    authoring::install_law_custody_for(&mut catalog, &["AudioExactTimeOffset"]).unwrap();
    assert_eq!(
        catalog.native_law_witness("AudioExactTimeOffset"),
        Some(witness)
    );
    let mut forged = witness.checked_type().clone();
    forged.invariants.clear();
    let mut foreign = StartupCatalog::new();
    foreign
        .insert_checked_native_type("AudioExactTimeOffset", &forged)
        .unwrap();
    let before = foreign.clone();
    assert!(authoring::install_law_custody_for(&mut foreign, &["AudioExactTimeOffset"]).is_err());
    assert_eq!(foreign, before);
}

#[test]
fn request_bounds_refuse_before_owner_preparation() {
    let mut catalog = StartupCatalog::new();
    let before = catalog.clone();
    assert!(
        authoring::install_law_custody_for(&mut catalog, &vec!["AudioExactTimeOffset"; 4097])
            .is_err()
    );
    assert!(authoring::install_law_custody_for(
        &mut catalog,
        &["AudioExactTimeOffset", "AudioExactTimeOffset"]
    )
    .is_err());
    assert_eq!(catalog, before);
}
