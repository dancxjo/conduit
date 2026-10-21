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
    typography: Typography,
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
    let chrome = chrome::Chrome::prepare_selected(screen, scale, typography)?;
    let bounds = LayoutRect {
        x: SPACE_XL as i16,
        y: chrome.top as i16,
        width: screen.width - SPACE_XL * 2,
        height: chrome.bottom - chrome.top,
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
    let label_height = TextLayout::new_selected(
        "",
        typography.style(GraphicsTextRole::Label.into()),
        bounds.width - 16,
        scale,
    )
    .map_err(|_| FaceSceneError::InvalidExtent)?
    .finish_height() as u16;
    let body_line = TextLayout::new_selected(
        "",
        typography.style(GraphicsTextRole::Body.into()),
        bounds.width - 16,
        scale,
    )
    .map_err(|_| FaceSceneError::InvalidExtent)?
    .finish_height() as u16;
    // Keep a complete measured caption and at least one text/caret line. Only
    // the old fixed gaps shrink under pressure; the border retains one pixel.
    let gaps = bounds
        .height
        .checked_sub(label_height + body_line)
        .filter(|gaps| *gaps >= 2)
        .ok_or(FaceSceneError::InvalidExtent)?
        .min(24);
    let outer = (gaps / 3).clamp(1, 8);
    let between = gaps - outer * 2;
    let content = LayoutRect {
        x: bounds.x + 8,
        y: bounds.y + (outer + label_height + between) as i16,
        width: bounds.width - 16,
        height: bounds.height - label_height - gaps,
    };
    let height = TextLayout::new_selected(
        text,
        typography.style(GraphicsTextRole::Body.into()),
        content.width,
        scale,
    )
    .map_err(|_| FaceSceneError::InvalidExtent)?
    .finish_height();
    let scroll = height.saturating_sub(u32::from(content.height));
    let label = LayoutRect {
        x: bounds.x + 8,
        y: bounds.y + outer as i16,
        width: bounds.width - 16,
        height: label_height,
    };
    push(
        scene,
        GraphicsCommand::text(
            label,
            screen,
            GraphicsPaintRole::Accent,
            &caption(
                &subject.name,
                start != 0 || scroll != 0,
                label.width,
                label.height,
                scale,
                typography,
            )?,
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
    let (x, y, line_height) = TextLayout::new_selected(
        text,
        typography.style(GraphicsTextRole::Body.into()),
        content.width,
        scale,
    )
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

/// A one-line renderer caption has explicit continuation; the exact original
/// name and complete document remain in the retained Face's Details. Work is
/// bounded by the existing 192-byte command limit, using actual glyph metrics.
fn caption(
    name: &str,
    at_end: bool,
    width: u16,
    height: u16,
    scale: u16,
    typography: Typography,
) -> Result<String, FaceSceneError> {
    let suffix = if at_end { " (end)" } else { "" };
    let mut end = name
        .len()
        .min(conduit_face::MAX_GRAPHICS_TEXT_BYTES - suffix.len());
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    loop {
        let candidate = alloc::format!(
            "{}{}{}",
            &name[..end],
            if end < name.len() { "…" } else { "" },
            suffix
        );
        if candidate.len() <= conduit_face::MAX_GRAPHICS_TEXT_BYTES
            && TextLayout::new_selected(
                &candidate,
                typography.style(GraphicsTextRole::Label.into()),
                width,
                scale,
            )
            .map_err(|_| FaceSceneError::InvalidExtent)?
            .finish_height()
                <= u32::from(height)
        {
            return Ok(candidate);
        }
        if end == 0 {
            return Err(FaceSceneError::InvalidExtent);
        }
        end -= 1;
        while !name.is_char_boundary(end) {
            end -= 1;
        }
    }
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
