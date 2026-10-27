//! Complete current producer, protected codec and native Collection realization.
//! CPU scanout is distinct from the normal-product QEMU receipt.
use super::*;
use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
use conduit_face::{ComponentVisualSelection, CrecheComponentRecipe, VisualTypography};

fn prepare(recipe: CrecheComponentRecipe) -> NativeFaceMask {
    let provider = crate::product_bases::fixture_surface_provider();
    NativeFaceMask::prepare_collection_creche(
        "host/native".into(),
        "boot/native".into(),
        OfferGeneration(1),
        "build",
        provider.entry.base_id.clone(),
        "surface/collection-creche",
        &provider,
        recipe,
        ComponentVisualSelection::default(),
        VisualTypography::Editorial,
        false,
    )
    .unwrap()
}
fn deep() -> BirthDraft {
    BirthDraft::new(
        "550e8400-e29b-41d4-a716-446655440000".into(),
        (0..16)
            .map(|i| BirthPlotChoice {
                title: alloc::format!("界{} {i}", "é".repeat(20)),
                search_text: "界".repeat(500),
                plot: conduit_body::ResidentPlot::new(
                    alloc::format!("source/{i}/{}", "界".repeat(15)).into(),
                    alloc::format!("checked/{i}/{}", "界".repeat(15)).into(),
                ),
                selected: i % 2 == 0,
                refusal: None,
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn complete_five_and_sixteen_unicode_faces_cross_matching_collection_storage() {
    for (name, draft) in [("default-five", tests::draft()), ("deep-sixteen", deep())] {
        for recipe in [
            CrecheComponentRecipe::SingleColumn,
            CrecheComponentRecipe::ResponsiveColumns,
        ] {
            let mut mask = prepare(recipe);
            let face = draft.face(&mask.birth_basis("collection/current")).unwrap();
            let bytes = conduitos_creche_core::data::encode(&face).unwrap();
            let roundtrip: Face = conduitos_creche_core::data::decode(&bytes).unwrap();
            assert_eq!(roundtrip, face);
            let linear = conduit_face::render_linear_face(&face).unwrap();
            let mut display = tests::CountingDisplay::default();
            mask.present(roundtrip, 1, 1, &mut display).unwrap();
            assert_eq!(mask.scene().unwrap().face(), &face);
            assert_eq!(
                conduit_face::render_linear_face(mask.scene().unwrap().face()).unwrap(),
                linear
            );
            assert_eq!(mask.component_fallback(), None);
            let producer = mask.publication().unwrap();
            let renderer = mask.render_receipt().unwrap();
            assert_eq!(producer.encoded_bytes as usize, bytes.len());
            assert_eq!(renderer.renderer_input_bytes as usize, bytes.len());
            assert!(renderer.renderer_show_value_bytes > 0);
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
            assert_eq!(mask.show().unwrap().face_id, face.identity);
            assert_eq!(renderer.face_id, face.identity.as_str());
            assert_ne!(producer.producer_plan_id, renderer.mask_plan_id);
            std::println!(
                "COLLECTION_MEASURE {name} {recipe:?} face={} producer={:?} renderer={:?}",
                bytes.len(),
                producer.collection_storage,
                renderer.collection_storage
            );
            if name == "deep-sixteen" {
                assert!(bytes.len() > crate::native_face_snapshot::MAX_SNAPSHOT_BYTES);
                let (mut smaller, _) = tests::fixture();
                let before = display.writes;
                assert_eq!(
                    smaller.present(
                        draft
                            .face(&smaller.birth_basis("collection/current"))
                            .unwrap(),
                        1,
                        1,
                        &mut display
                    ),
                    Err(NativeFaceMaskError::Producer(FaceSnapshotRefusal::Capacity))
                );
                assert!(smaller.show().is_none());
                assert_eq!(display.writes, before);
            }
            mask.suspend().unwrap();
            assert!(mask.show().is_none());
            assert!(mask.route_keyboard().is_err());
        }
    }
}
