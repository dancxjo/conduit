use conduit_alife::{ReactionDiffusionRegion, ReactionDiffusionRegionId};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn reaction_diffusion_region_round_trips_exact_geometry_extrema() {
    for region in [
        ReactionDiffusionRegion::new(
            ReactionDiffusionRegionId::new(0).unwrap(),
            0,
            u16::MAX,
            0,
            u16::MAX,
        )
        .unwrap(),
        ReactionDiffusionRegion::new(
            ReactionDiffusionRegionId::new(u16::MAX).unwrap(),
            u16::MAX,
            0,
            u16::MAX,
            0,
        )
        .unwrap(),
    ] {
        let structured = region.into_structured().unwrap();
        assert_eq!(
            ReactionDiffusionRegion::from_structured(structured).unwrap(),
            region
        );
    }
}

#[test]
fn reaction_diffusion_region_preserves_authored_field_order() {
    let region =
        ReactionDiffusionRegion::new(ReactionDiffusionRegionId::new(7).unwrap(), 11, 13, 17, 19)
            .unwrap();
    assert_eq!(*region.region_id().get(), 7);
    assert_eq!(
        (
            region.origin_x(),
            region.origin_y(),
            region.width(),
            region.height(),
        ),
        (11, 13, 17, 19)
    );
}
