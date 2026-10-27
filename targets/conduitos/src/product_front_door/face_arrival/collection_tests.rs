//! Product selection and complete observer envelope from actual completed storage.
use super::*;
#[test]
fn local_arrival_selects_collection_before_observing_face_and_rejects_stale_evidence() {
    let provider = crate::product_bases::fixture_surface_provider();
    let mut arrival = FaceArrival::prepare(
        "host/native".into(),
        "boot/native".into(),
        OfferGeneration(1),
        "build",
        provider.entry.base_id.clone(),
        &provider,
        None,
    )
    .unwrap();
    let face = crate::native_face_mask::tests::default_creche_face(&arrival.basis);
    arrival.present(face.clone(), &mut tests::Display).unwrap();
    let producer = arrival.mask.publication().unwrap();
    let renderer = arrival.mask.render_receipt().unwrap();
    assert_eq!(
        producer
            .collection_storage
            .as_ref()
            .unwrap()
            .maximum_value_bytes,
        256 * 1024
    );
    assert_eq!(
        renderer
            .collection_storage
            .as_ref()
            .unwrap()
            .maximum_value_bytes,
        256 * 1024
    );
    let show = arrival.mask.show().unwrap();
    let mut stale = producer.clone();
    stale.face_revision += 1;
    assert_eq!(
        super::super::native_action_evidence::face_with_storage(
            &face,
            show,
            1,
            1,
            Some((&stale, renderer))
        ),
        Err("native-action-evidence-stale-storage")
    );
}
