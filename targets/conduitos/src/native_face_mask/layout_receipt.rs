//! CPU producer/compositor custody proof; no guest scanout or owner mutation.
use crate::native_face_scene::raster_layout_tests::Pixels;
use alloc::vec::Vec;
use conduit_face::Face;

#[test]
fn cpu_native_mask_reflow_retains_face_and_retires_old_show() {
    use crate::native_face_mask::NativeFaceMask;
    let face: Face = serde_json::from_str(include_str!(
        "../../../../proof/conformance/face-waist/domain-derived/lulled-body-face.json"
    ))
    .unwrap();
    let provider = crate::product_bases::fixture_surface_provider();
    let mut mask = NativeFaceMask::prepare(
        "host/layout-cpu".into(),
        "boot/layout-cpu".into(),
        conduit_core::OfferGeneration(1),
        "layout-proof",
        provider.entry.base_id.clone(),
        "surface/layout-cpu",
        &provider,
    )
    .unwrap();
    let mut prior = None;
    for (index, (width, height)) in [(1280u16, 800u16), (640, 480), (320, 240)]
        .into_iter()
        .enumerate()
    {
        let mut pixels = Pixels::new(width, height);
        let sequence = index as u64 + 1;
        let receipt = mask
            .present(face.clone(), sequence, sequence, &mut pixels)
            .unwrap();
        let publication = mask.publication().unwrap();
        let show = mask.show().unwrap();
        assert_eq!(receipt.face_id, face.identity);
        assert_eq!(publication.face_id, face.identity.as_str());
        assert!(publication.kernel_signs > 0);
        assert_eq!(show.show.face_id, face.identity);
        assert_eq!(show.show.plan_id, receipt.plan_id);
        assert_eq!(show.show.active_play_id, receipt.active_play_id);
        assert_eq!(show.show.manifestation_id, receipt.manifestation_id);
        assert!(receipt.display.pixels_written > 0);
        assert!(usize::from(receipt.display.commands) <= conduit_face::MAX_GRAPHICS_COMMANDS);
        if let Some(prior) = prior {
            assert_ne!(show.show_id, prior);
        }
        prior = Some(show.show_id.clone());
        let scene = mask.scene().unwrap();
        assert_eq!(scene.face(), &face);
        let frame = scene.frame().unwrap();
        let commands:Vec<_>=frame.scene.commands().iter().map(|command|serde_json::json!({
            "kind":alloc::format!("{:?}",command.kind),"role":alloc::format!("{:?}",command.text_role()),
            "text":command.payload(),"bounds":{"x":command.bounds.x,"y":command.bounds.y,
                "width":command.bounds.width,"height":command.bounds.height},
        })).collect();
        std::println!(
            "NATIVE_LAYOUT_COMPOSITION={}",
            serde_json::json!({
                "proof_class":"CPU production Mask/producer/compositor execution with immutable owner Face and fixture surface provider; no QEMU or domain action",
                "width":width,"height":height,"scale":scene.text_scale_percent(),
                "publication":publication,"show":show,"scene_commands":commands,
                "composition":{"face_id":receipt.face_id,"manifestation_id":receipt.manifestation_id,
                    "plan_id":receipt.plan_id,"active_play_id":receipt.active_play_id,"play_sequence":receipt.play_sequence,
                    "placement_id":receipt.placement_id,"host_id":receipt.host_id,"boot_id":receipt.boot_id,
                    "offer_generation":receipt.offer_generation,"surface_id":receipt.surface_id,
                    "display_base_id":receipt.display_base_id,"presenter_implementation_id":receipt.presenter_implementation_id,
                    "presenter_capability_id":receipt.presenter_capability_id,"presenter_artifact_id":receipt.presenter_artifact_id,
                    "commands":receipt.display.commands,"pixels_written":receipt.display.pixels_written},
            })
        );
        mask.suspend().unwrap();
        assert!(mask.show().is_none());
    }
}
