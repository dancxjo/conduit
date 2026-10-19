//! Observed geometry must describe real current frame hit regions.
use super::tests::face;
use super::*;

#[test]
fn observed_display_bounds_contain_real_control_hits() {
    for (width, height) in [(320, 240), (640, 480), (1280, 800)] {
        let scene = NativeFaceScene::prepare(face(1), width, height).unwrap();
        let bounds = scene.bounds();
        assert_eq!(
            (bounds.x, bounds.y, bounds.width, bounds.height),
            (0, 0, width, height)
        );
        for hit in scene.frame().unwrap().hits {
            assert!(hit.bounds.x >= bounds.x && hit.bounds.y >= bounds.y);
            assert!(
                i32::from(hit.bounds.x) + i32::from(hit.bounds.width) <= i32::from(bounds.width)
            );
            assert!(
                i32::from(hit.bounds.y) + i32::from(hit.bounds.height) <= i32::from(bounds.height)
            );
        }
    }
}
