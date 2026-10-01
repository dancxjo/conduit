use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::{LayoutError, LayoutFrame, LayoutRect};

#[test]
fn layout_values_round_trip_through_their_native_owner() {
    let viewport = LayoutRect {
        x: i16::MIN,
        y: i16::MAX,
        width: u16::MAX,
        height: 0,
    };
    let frame = LayoutFrame {
        viewport,
        child_count: 1,
        children: [viewport; 8],
    };
    let structured = frame.into_structured().unwrap();
    assert_eq!(LayoutFrame::from_structured(structured).unwrap(), frame);

    for refusal in [
        LayoutError::TooManyChildren,
        LayoutError::ExtentOutOfBounds,
        LayoutError::UndersizedExtent,
        LayoutError::CoordinateOverflow,
        LayoutError::MalformedEncoding,
        LayoutError::NonCanonicalEncoding,
    ] {
        let structured = refusal.into_structured().unwrap();
        assert_eq!(LayoutError::from_structured(structured).unwrap(), refusal);
    }
}

#[test]
fn layout_family_has_no_handwritten_duplicates() {
    let source = include_str!("../src/layout.rs");
    assert!(!source.contains("pub struct LayoutRect"));
    assert!(!source.contains("pub struct LayoutFrame"));
    assert!(!source.contains("pub enum LayoutError"));
}
