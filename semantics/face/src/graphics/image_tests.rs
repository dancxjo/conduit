use super::*;
use crate::GraphicsScene;

fn bounds() -> LayoutRect {
    LayoutRect {
        x: 0,
        y: 0,
        width: 320,
        height: 240,
    }
}

#[test]
fn exact_generation_round_trips_without_text_or_storage_growth() {
    let key = GraphicsImageKey::new([255; 32], [128; 32]).unwrap();
    let command = GraphicsCommand::image(bounds(), bounds(), key).unwrap();
    assert_eq!(command.image_key(), Some(key));
    assert_eq!(command.payload(), "");
    let mut scene = GraphicsScene::empty();
    scene.push(command).unwrap();
    let encoded = scene.encode();
    assert_eq!(encoded[0], 4);
    assert_eq!(scene.encoded_len(), 2 + 21 + 64);
    assert_eq!(
        GraphicsScene::decode(&encoded[..scene.encoded_len()]).unwrap(),
        scene
    );
    let mut old = encoded;
    old[0] = 2;
    assert!(GraphicsScene::decode(&old[..scene.encoded_len()]).is_err());
    assert_eq!(super::super::MAX_GRAPHICS_COMMANDS, 48);
    assert_eq!(super::super::MAX_GRAPHICS_TEXT_BYTES, 192);
}

#[test]
fn missing_generations_and_noncanonical_paint_refuse() {
    assert_eq!(
        GraphicsImageKey::new([0; 32], [1; 32]),
        Err(GraphicsError::InvalidImageKey)
    );
    assert_eq!(
        GraphicsImageKey::new([1; 32], [0; 32]),
        Err(GraphicsError::InvalidImageKey)
    );
    assert_eq!(
        GraphicsImageKey::decode(&[1; 63]),
        Err(GraphicsError::InvalidImageKey)
    );
    let key = GraphicsImageKey::new([1; 32], [2; 32]).unwrap();
    assert!(validate(
        GraphicsPaintRole::Accent,
        GraphicsShapeStyle::Fill,
        &key.encode()
    )
    .is_err());
    let mut invalid = bounds();
    invalid.width = 0;
    assert!(GraphicsCommand::image(invalid, bounds(), key).is_err());
}
