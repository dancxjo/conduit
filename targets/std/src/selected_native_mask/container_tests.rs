//! Pure validation proof only: no enclosing owner, reservation, SDL or Show.
use super::*;
use NativeMaskContainerMode::{Fullscreen, Tile};
use NativeMaskContainerRefusal as Refusal;

fn rectangle(x: i16, y: i16, width: u16, height: u16) -> LayoutRect {
    LayoutRect {
        x,
        y,
        width,
        height,
    }
}
fn backing() -> NativeMaskContainerBacking {
    NativeMaskContainerBacking {
        scene: 20,
        frame: 30,
        compositor: 40,
    }
}
fn check(
    rectangle: LayoutRect,
    mode: NativeMaskContainerMode,
) -> Result<CheckedNativeMaskContainer, Refusal> {
    CheckedNativeMaskContainer::check((1024, 600), rectangle, mode, backing(), 90)
}

#[test]
fn strict_tile_retains_original_output_and_independent_local_extent() {
    let tile = check(rectangle(32, 24, 320, 320), Tile).unwrap();
    assert_eq!(tile.original_raster_extent(), (1024, 600));
    assert_eq!(tile.local_scene_extent(), (320, 320));
    assert_eq!(tile.rectangle(), rectangle(32, 24, 320, 320));
    assert_eq!(tile.required_backing_bytes(), 90);
    assert_eq!(
        check(rectangle(0, 0, 1024, 600), Tile),
        Err(Refusal::TileNotStrict)
    );
    // Sharing an edge is valid; strict means proper subset, not an invented margin.
    assert!(check(rectangle(0, 0, 1024, 320), Tile).is_ok());
}

#[test]
fn fullscreen_is_exactly_the_original_output() {
    let full = check(rectangle(0, 0, 1024, 600), Fullscreen).unwrap();
    assert_eq!(full.original_raster_extent(), (1024, 600));
    assert_eq!(full.local_scene_extent(), (1024, 600));
    assert_eq!(
        check(rectangle(1, 0, 1023, 600), Fullscreen),
        Err(Refusal::FullscreenMismatch)
    );
    assert_eq!(
        check(rectangle(0, 0, 640, 480), Fullscreen),
        Err(Refusal::FullscreenMismatch)
    );
}

#[test]
fn invalid_or_unsupported_container_geometry_has_precise_refusal() {
    for (rectangle, reason) in [
        (rectangle(0, 0, 0, 320), Refusal::ZeroExtent),
        (rectangle(0, 0, 320, 0), Refusal::ZeroExtent),
        (rectangle(-1, 0, 320, 320), Refusal::NegativeOrigin),
        (rectangle(0, -1, 320, 320), Refusal::NegativeOrigin),
        (rectangle(800, 0, 320, 320), Refusal::OutsideOriginalRaster),
        (rectangle(0, 400, 320, 320), Refusal::OutsideOriginalRaster),
        (rectangle(0, 0, 319, 320), Refusal::UnsupportedSceneExtent),
        (rectangle(0, 0, 320, 239), Refusal::UnsupportedSceneExtent),
        (
            rectangle(i16::MAX, 0, u16::MAX, 320),
            Refusal::CoordinateOverflow,
        ),
    ] {
        assert_eq!(check(rectangle, Tile), Err(reason));
    }
    assert_eq!(
        CheckedNativeMaskContainer::check((0, 600), rectangle(0, 0, 320, 320), Tile, backing(), 90),
        Err(Refusal::ZeroExtent)
    );
    assert_eq!(
        CheckedNativeMaskContainer::check(
            (40_000, 600),
            rectangle(0, 0, 20_000, 320),
            Tile,
            backing(),
            90
        ),
        Err(Refusal::UnsupportedSceneExtent)
    );
}

#[test]
fn geometry_does_not_replace_finite_backing_validation_or_mutate_prior_choice() {
    let prior = check(rectangle(0, 0, 1024, 600), Fullscreen).unwrap();
    let attempted = CheckedNativeMaskContainer::check(
        (1024, 600),
        rectangle(20, 20, 320, 320),
        Tile,
        backing(),
        89,
    );
    assert_eq!(attempted, Err(Refusal::Unfunded));
    assert_eq!(prior.rectangle(), rectangle(0, 0, 1024, 600));
    assert_eq!(
        CheckedNativeMaskContainer::check(
            (1024, 600),
            rectangle(20, 20, 320, 320),
            Tile,
            NativeMaskContainerBacking {
                scene: usize::MAX,
                frame: 1,
                compositor: 0
            },
            usize::MAX
        ),
        Err(Refusal::BackingOverflow)
    );
}
