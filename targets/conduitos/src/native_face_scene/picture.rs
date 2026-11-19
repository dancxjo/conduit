//! The selected photographic opening page borrows the checked Face and its
//! prepared reader text. All geometry and indices have fixed size.
use super::*;
use conduit_face::{FacePropertyValue, FaceRole};

#[derive(Clone, Copy)]
pub(super) struct PicturePage {
    date: usize,
    time: usize,
    weather: usize,
    panel: LayoutRect,
    lines: [LayoutRect; 3],
}

const ROLES: [GraphicsTextRole; 3] = [
    GraphicsTextRole::Label,
    GraphicsTextRole::Value,
    GraphicsTextRole::Body,
];

fn property_text(face: &Face, index: usize) -> Option<&str> {
    match &face.properties.get(index)?.value {
        FacePropertyValue::Text(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

impl NativeFaceScene {
    /// Open an admitted photograph as a cover page. All original reading pages,
    /// disclosure and action mappings remain reachable through ordinary paging.
    pub fn enable_picture_frame(&mut self) -> Result<(), FaceSceneError> {
        if self.picture_frame.is_none()
            && let Some(prepared) = self.prepare_picture()
        {
            for row in &mut self.primary {
                row.page += 1;
            }
            self.picture_frame = Some(prepared);
        }
        Ok(())
    }

    fn prepare_picture(&self) -> Option<PicturePage> {
        let face = &self.face;
        let semantic = |name: &str| {
            let mut matches = face.subjects.iter().enumerate().filter(|(_, subject)| {
                matches!(&subject.role, FaceRole::Semantic(kind) if kind.as_str() == name)
            });
            let found = matches.next()?;
            matches.next().is_none().then_some(found)
        };
        let (_, image) = semantic("media/image")?;
        if face.properties.get(self.image?.1)?.subject != image.identity {
            return None;
        }
        let (_, clock) = semantic("time/civil-observation")?;
        let property = |name: &str| {
            face.properties
                .iter()
                .position(|p| p.subject == clock.identity && p.name == name)
        };
        let date = property("local-date")?;
        let time = property("local-time")?;
        let (weather_subject, _) = semantic("weather/conditions")?;
        // The first Body wording is the complete owner or localized weather
        // summary made by document::prepare. Attributions follow it.
        let weather = self.primary.iter().position(|r| {
            r.subject_index == Some(weather_subject) && r.role == GraphicsTextRole::Body
        })?;
        if !self.primary[weather].whole_wording {
            return None;
        }
        let texts = [
            property_text(face, date)?,
            property_text(face, time)?,
            self.primary[weather].text.as_str(),
        ];
        let width = self.screen.width;
        let height = self.screen.height;
        if width < 400 || height < 300 {
            return None;
        }
        let margin = (width / 28).clamp(14, 42);
        let pad = (width / 50).clamp(12, 24);
        let usable = width.checked_sub(2 * margin + 2 * pad)?;
        let mut max_line_width = 0;
        let mut heights = [0u16; 3];
        for ((text, role), line_height) in texts.into_iter().zip(ROLES).zip(&mut heights) {
            let style = self.typography.style(role.into());
            let metrics = crate::display::typography::intrinsic_text_metrics(
                text,
                style,
                self.text_scale_percent,
            )
            .ok()?;
            let line_width = u16::try_from(metrics.preferred_inline).ok()?.min(usable);
            max_line_width = max_line_width.max(line_width);
            *line_height = u16::try_from(
                TextLayout::new_selected(text, style, usable, self.text_scale_percent)
                    .ok()?
                    .finish_height(),
            )
            .ok()?;
        }
        let gap = 6;
        let panel_height = heights
            .iter()
            .try_fold(2 * pad + 2 * gap, |total: u16, h| total.checked_add(*h))?;
        if panel_height > height / 2 {
            return None;
        }
        let y = height - margin - panel_height;
        let panel = LayoutRect {
            x: margin as i16,
            y: y as i16,
            width: max_line_width + 2 * pad,
            height: panel_height,
        };
        let mut cursor = y + pad;
        let lines = heights.map(|h| {
            let rect = LayoutRect {
                x: (margin + pad) as i16,
                y: cursor as i16,
                width: max_line_width,
                height: h,
            };
            cursor += h + gap;
            rect
        });
        Some(PicturePage {
            date,
            time,
            weather,
            panel,
            lines,
        })
    }

    pub(super) fn picture_page(&self) -> Result<Option<FaceFrame<'_>>, FaceSceneError> {
        let Some(page) = self.picture_frame else {
            return Ok(None);
        };
        if self.page != 0
            || self.showing_details
            || self.showing_diagram
            || self.has_pending_input()
            || self.local_notice.is_some()
        {
            return Ok(None);
        }
        let mut scene = GraphicsScene::empty();
        let screen = self.screen;
        let (image, _) = self.image.ok_or(FaceSceneError::InvalidFace)?;
        push(&mut scene, GraphicsCommand::image(screen, screen, image))?;
        push(
            &mut scene,
            GraphicsCommand::rect(
                page.panel,
                screen,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            ),
        )?;
        let texts = [
            property_text(&self.face, page.date),
            property_text(&self.face, page.time),
            Some(self.primary[page.weather].text.as_str()),
        ];
        for (((text, role), bounds), paint) in texts.into_iter().zip(ROLES).zip(page.lines).zip([
            GraphicsPaintRole::Muted,
            GraphicsPaintRole::Foreground,
            GraphicsPaintRole::Foreground,
        ]) {
            push(
                &mut scene,
                GraphicsCommand::text(
                    bounds,
                    screen,
                    paint,
                    text.ok_or(FaceSceneError::InvalidFace)?,
                )
                .and_then(|command| command.with_text_role(role)),
            )?;
        }
        if let Some(controls) = self.page_controls()? {
            let label = "Next page";
            let style = self.typography.style(GraphicsTextRole::Label.into());
            let metrics = crate::display::typography::intrinsic_text_metrics(
                label,
                style,
                self.text_scale_percent,
            )
            .map_err(|_| FaceSceneError::InvalidExtent)?;
            let width = (metrics.preferred_inline + 12).min(u32::from(controls.next.width)) as u16;
            let hint = LayoutRect {
                x: controls.next.x + (controls.next.width - width) as i16,
                width,
                ..controls.next
            };
            push(
                &mut scene,
                GraphicsCommand::rect(
                    hint,
                    screen,
                    GraphicsPaintRole::Background,
                    GraphicsShapeStyle::Fill,
                ),
            )?;
            push(
                &mut scene,
                GraphicsCommand::text(hint, screen, GraphicsPaintRole::Foreground, label)
                    .and_then(|command| command.with_text_role(GraphicsTextRole::Label)),
            )?;
        }
        Ok(Some(FaceFrame {
            scene,
            face_id: &self.face.identity,
            revision: self.face.revision,
            page: 0,
            pages: self.pages(),
            hits: Vec::new(),
            continuation: None,
        }))
    }
}
