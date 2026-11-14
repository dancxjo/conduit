use super::*;

#[test]
fn actual_single_surface_backing_refuses_before_allocation_and_binds_exact_slot() {
    let required = PreparedCompositorBacking::required_bytes(640, 480).unwrap();
    assert!(matches!(
        PreparedCompositorBacking::prepare("surface", 640, 480, required - 1),
        Err(NativeCompositorError::SurfaceCapacityExceeded)
    ));
    let backing = PreparedCompositorBacking::prepare("surface", 640, 480, required).unwrap();
    let pointer = backing.surfaces[0].buffer.pixels.as_ptr();
    assert_eq!(backing.backing_bytes(), required);
    let admission = CompositorAdmission::new(
        "host".into(),
        "boot".into(),
        conduit_core::OfferGeneration(1),
        "renderer".into(),
        "display".into(),
        alloc::vec!["placement".into()],
        alloc::vec!["surface".into()],
    )
    .unwrap();
    let compositor = backing.bind(admission.clone()).unwrap();
    assert_eq!(compositor.surfaces[0].buffer.pixels.as_ptr(), pointer);
    assert_eq!(compositor.allocated_pixels(), 640 * 480);
    let mut wrong = admission;
    wrong.surface_ids[0] = "other".into();
    assert!(matches!(
        PreparedCompositorBacking::prepare("surface", 640, 480, required)
            .unwrap()
            .bind(wrong),
        Err(NativeCompositorError::UnadmittedSurface)
    ));
}
