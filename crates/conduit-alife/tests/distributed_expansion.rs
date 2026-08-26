use conduit_alife::{
    expanded_three_region_lenia, LENIA_JOIN_KIND, LENIA_PARTITION_KIND, LENIA_REGION_STEP_KIND,
    SCALAR_FIELD_GRAY8_KIND,
};

#[test]
fn unchanged_portable_lenia_expands_to_three_workers_and_six_typed_cords() {
    let expanded = expanded_three_region_lenia().unwrap();
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == LENIA_PARTITION_KIND));
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == LENIA_JOIN_KIND));
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == LENIA_REGION_STEP_KIND)
            .count(),
        3
    );
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == SCALAR_FIELD_GRAY8_KIND));
    assert!(expanded
        .gears
        .iter()
        .any(|gear| { gear.kind_id.as_str() == conduit_std_catalog::BITMAP_PRESENTATION_KIND }));
    assert_eq!(expanded.realization_backs.len(), 2);
    assert!(expanded
        .realization_backs
        .iter()
        .any(|back| { back.kind_id.as_str() == conduit_std_catalog::LENIA_STEP_KIND }));
    assert!(expanded.realization_backs.iter().any(|back| {
        back.kind_id.as_str() == conduit_std_catalog::SCALAR_FIELD_PRESENTATION_KIND
    }));
}
