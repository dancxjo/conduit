//! This post-factory observation is deliberately not a decoder funding claim.
use super::*;
#[test]
fn retained_preparation_counts_visual_spare_capacity_without_changing_look() {
    let (_, configuration, face) = fixture();
    let mut prepared = PreparedConfiguredScene::prepare(&configuration, &face).unwrap();
    let before = prepared.retained_backing_bytes().unwrap();
    assert!(core::ptr::eq(prepared.structure.face(), &face));
    let identity = prepared.look.visual.expression.identity.clone();
    let capacity = prepared.look.visual.expression.identity.capacity();
    prepared.look.visual.expression.identity.reserve(79);
    let additional = prepared.look.visual.expression.identity.capacity() - capacity;
    assert_eq!(prepared.look.visual.expression.identity, identity);
    assert_eq!(prepared.retained_backing_bytes(), Some(before + additional));
    assert_eq!(
        prepared.look.requested,
        configuration.requested_look().unwrap()
    );
}
#[test]
fn retained_preparation_includes_full_original_reading_and_native_structure() {
    let (_, configuration, face) = fixture();
    let prepared = PreparedConfiguredScene::prepare(&configuration, &face).unwrap();
    let reading = &prepared.structure.recipe().reading;
    let reading_bytes = reading.source_face_identity.capacity()
        + reading.clauses.capacity() * core::mem::size_of::<FaceUtteranceClause>()
        + reading
            .clauses
            .iter()
            .map(|c| c.text.capacity())
            .sum::<usize>();
    assert!(prepared.retained_backing_bytes().unwrap() > reading_bytes);
    assert!(
        reading
            .clauses
            .iter()
            .any(|clause| clause.text.contains("Original fallback information"))
    );
    assert_eq!(
        prepared.look.requested.identity(),
        prepared.look.selected.identity()
    );
}
