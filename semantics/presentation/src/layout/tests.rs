use super::*;

#[test]
fn row_rounding_is_stable_and_encoding_is_canonical() {
    let frame = LayoutFrame::viewport(10, 4, 3, 2, 2)
        .unwrap()
        .distribute(LayoutAxis::Horizontal, 1)
        .unwrap();
    assert_eq!(
        frame.children[0],
        LayoutRect {
            x: 0,
            y: 0,
            width: 3,
            height: 4
        }
    );
    assert_eq!(
        frame.children[1],
        LayoutRect {
            x: 4,
            y: 0,
            width: 3,
            height: 4
        }
    );
    assert_eq!(
        frame.children[2],
        LayoutRect {
            x: 8,
            y: 0,
            width: 2,
            height: 4
        }
    );
    let encoded = frame.encode();
    assert_eq!(
        LayoutFrame::decode(&encoded[..frame.encoded_len()]),
        Ok(frame)
    );
    assert_eq!(
        LayoutFrame::decode(&encoded),
        Err(LayoutError::NonCanonicalEncoding)
    );
}

#[test]
fn zero_maximum_undersized_clipping_and_alignment_are_exact() {
    assert_eq!(
        LayoutFrame::viewport(8, 8, 0, 0, 0)
            .unwrap()
            .distribute(LayoutAxis::Vertical, 7)
            .unwrap()
            .child_count,
        0
    );
    let maximum = LayoutFrame::viewport(32, 16, MAX_LAYOUT_CHILDREN as u8, 40, 40)
        .unwrap()
        .inset(2)
        .unwrap();
    assert_eq!(
        maximum.children[0],
        LayoutRect {
            x: 2,
            y: 2,
            width: 28,
            height: 12
        }
    );
    assert_eq!(
        maximum.distribute(LayoutAxis::Horizontal, 5),
        Err(LayoutError::UndersizedExtent)
    );
    let aligned = LayoutFrame::viewport(9, 9, 1, 4, 2)
        .unwrap()
        .align(LayoutAlignment::Center, LayoutAlignment::End)
        .unwrap();
    assert_eq!(
        aligned.children[0],
        LayoutRect {
            x: 2,
            y: 7,
            width: 4,
            height: 2
        }
    );
}
