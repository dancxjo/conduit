use super::*;

#[test]
fn capture_dma_checks_every_buffer_extent_including_segment_edges() {
    for (ring, buffers) in [(0x40000, 0x10000), (0x40000, 0xf000), (0x40000, 0xf800)] {
        assert!(validate_geometry::<8>(ring, buffers).is_ok());
    }
    assert!(validate_geometry::<1>(0x40000, 0xf800).is_ok());
    for (ring, buffers) in [
        (0, 0x10000),
        (0x40001, 0x10000),
        (0x4ff00, 0x10000),
        (0x40000, 0),
        (0x40000, 0xf001),
        (0x40000, 0xf801),
        (0x40000, 0x40000),
        (0x40000, 0x3f800),
        (u64::MAX & !63, 0x10000),
        (0x40000, u64::MAX - 2048),
    ] {
        assert!(matches!(
            validate_geometry::<8>(ring, buffers),
            Err(EndpointNativeRefusal::Mapping)
        ));
    }
    assert!(validate_geometry::<0>(0x40000, 0x10000).is_err());
    assert!(validate_geometry::<9>(0x40000, 0x10000).is_err());
}
