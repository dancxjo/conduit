use super::*;

#[test]
fn picture_frame_presents_wallpaper_without_inventing_clock_or_weather() {
    let mut face = fixture();
    let retained = |subject: &str| subject != CLOCK && subject != WEATHER;
    face.subjects.retain(|subject| retained(&subject.identity));
    face.properties
        .retain(|property| retained(&property.subject));
    face.text.retain(|wording| retained(&wording.subject));
    face.relationships
        .retain(|relation| retained(&relation.source) && retained(&relation.target));
    let face = refresh(face);
    let expression = picture_frame_expression().unwrap();

    for (width, height) in [(1024, 600), (640, 480)] {
        let mut scene = NativeFaceScene::prepare_picture_frame(
            face.clone(),
            width,
            height,
            true,
            &expression,
            None,
        )
        .unwrap();
        let frame = scene.frame().unwrap();
        assert_eq!(frame.face_id, &face.identity);
        let image = &frame.scene.commands()[0];
        assert_eq!(
            image.image_key(),
            Some(GraphicsImageKey::new([1; 32], [2; 32]).unwrap())
        );
        assert_eq!(image.bounds, scene.bounds());
        assert!(
            frame.scene.commands().iter().skip(1).all(|command| {
                command.payload().is_empty() || command.payload() == "Next page"
            })
        );
        assert!(scene.pages() > 1);
        scene.turn_page(true);
        assert_eq!(scene.frame().unwrap().page, 1);
        scene.turn_page(false);
        assert_eq!(scene.frame().unwrap().page, 0);
    }
}
