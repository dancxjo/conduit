//! Physical editor realization over immutable, authoritative Face text.
//! Editing availability is producer truth; focus geometry belongs to this Mask.
//! This renderer stores no text or editing state.
use super::*;
use conduit_face::{FacePropertyValue, FaceRole};

pub(super) fn append(
    scene: &mut GraphicsScene,
    face: &Face,
    screen: LayoutRect,
    scale: u16,
) -> Result<bool, FaceSceneError> {
    let Some(subject) = face.subjects.iter().find(|subject| {
        subject.role == FaceRole::Document
            && face.properties.iter().any(|property| {
                property.subject == subject.identity
                    && property.name == "text-editing-active"
                    && property.value == FacePropertyValue::Flag(true)
            })
    }) else {
        return Ok(false);
    };
    let text = face
        .text
        .iter()
        .find(|text| text.subject == subject.identity)
        .map_or("", |text| text.text.as_str());
    // The complete document stays in Face. A single admitted graphics payload
    // shows its tail with an explicit end notice.
    let mut start = text
        .len()
        .saturating_sub(conduit_face::MAX_GRAPHICS_TEXT_BYTES);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let text = &text[start..];
    let bounds = LayoutRect {
        x: SPACE_XL as i16,
        y: layout::HEADER as i16,
        width: screen.width - SPACE_XL * 2,
        height: screen.height - layout::HEADER - layout::FOOTER,
    };
    push(
        scene,
        GraphicsCommand::rect(
            bounds,
            screen,
            GraphicsPaintRole::Focus,
            GraphicsShapeStyle::Stroke,
        ),
    )?;
    let label_height =
        TextLayout::new_scaled("", GraphicsTextRole::Label.into(), bounds.width - 16, scale)
            .map_err(|_| FaceSceneError::InvalidExtent)?
            .finish_height() as u16;
    let content = LayoutRect {
        x: bounds.x + 8,
        y: bounds.y + label_height as i16 + 16,
        width: bounds.width - 16,
        height: bounds.height.saturating_sub(label_height + 24),
    };
    let height = TextLayout::new_scaled(text, GraphicsTextRole::Body.into(), content.width, scale)
        .map_err(|_| FaceSceneError::InvalidExtent)?
        .finish_height();
    let scroll = height.saturating_sub(u32::from(content.height));
    let label = LayoutRect {
        x: bounds.x + 8,
        y: bounds.y + 8,
        width: bounds.width - 16,
        height: label_height,
    };
    push(
        scene,
        GraphicsCommand::text(
            label,
            screen,
            GraphicsPaintRole::Accent,
            &if start == 0 && scroll == 0 {
                subject.name.clone()
            } else {
                alloc::format!("{} (end)", subject.name)
            },
        )
        .and_then(|command| command.with_text_role(GraphicsTextRole::Label)),
    )?;
    let text_bounds = LayoutRect {
        y: content.y - scroll as i16,
        height: height as u16,
        ..content
    };
    if !text.is_empty() {
        push(
            scene,
            GraphicsCommand::text(text_bounds, content, GraphicsPaintRole::Foreground, text)
                .and_then(|command| command.with_text_role(GraphicsTextRole::Body)),
        )?;
    }
    // The rasterizer owns wrapping, Unicode glyph advances and trailing lines.
    let (x, y, line_height) =
        TextLayout::new_scaled(text, GraphicsTextRole::Body.into(), content.width, scale)
            .map_err(|_| FaceSceneError::InvalidExtent)?
            .finish_caret();
    let caret_bounds = LayoutRect {
        x: content.x + x.min(u32::from(content.width.saturating_sub(2))) as i16,
        y: content.y + y as i16 - scroll as i16,
        width: 2,
        height: (line_height as u16).saturating_sub(4),
    };
    push(
        scene,
        GraphicsCommand::rect(
            caret_bounds,
            content,
            GraphicsPaintRole::Focus,
            GraphicsShapeStyle::Fill,
        ),
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{FaceProperty, FaceText, GraphicsCommandKind};

    #[test]
    fn immutable_unicode_document_has_a_clipped_visible_end_caret_at_bounded_extents() {
        for value in [
            String::new(),
            "café".into(),
            "é\n".repeat(85),
            "w".repeat(256),
        ] {
            for (width, height, scale) in [
                (320, 240, 100),
                (320, 240, 200),
                (640, 480, 100),
                (640, 480, 200),
            ] {
                let mut face = super::super::tests::face(1);
                face.properties.push(FaceProperty {
                    subject: "z-first".into(),
                    name: "text-editing-active".into(),
                    value: FacePropertyValue::Flag(true),
                });
                if !value.is_empty() {
                    face.text.push(FaceText {
                        subject: "z-first".into(),
                        text: value.clone(),
                    });
                }
                let before = face.clone();
                let mut scene = GraphicsScene::empty();
                assert!(
                    append(
                        &mut scene,
                        &face,
                        LayoutRect {
                            x: 0,
                            y: 0,
                            width,
                            height
                        },
                        scale
                    )
                    .unwrap()
                );
                assert_eq!(face, before);
                let caret = scene.commands().last().unwrap();
                assert_eq!(caret.kind, GraphicsCommandKind::Rect);
                assert_eq!(caret.paint, GraphicsPaintRole::Focus);
                assert_eq!(caret.bounds.width, 2);
                assert!(caret.bounds.y >= caret.clip.y);
                assert!(
                    i32::from(caret.bounds.y) + i32::from(caret.bounds.height)
                        <= i32::from(caret.clip.y) + i32::from(caret.clip.height)
                );
            }
        }
    }
}
