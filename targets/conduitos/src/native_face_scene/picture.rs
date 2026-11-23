//! The selected photographic opening page borrows the checked Face and its
//! prepared reader text. All geometry and indices have fixed size.
use super::*;
use alloc::format;
use conduit_core::{LocalDate, LocalDateTime, LocalTime, TemporalScale, UtcOffsetSeconds};
use conduit_face::{FacePropertyValue, FaceRole};

pub(super) struct PicturePage {
    image: conduit_face::GraphicsImageKey,
    date: String,
    time: String,
    period: &'static str,
    period_bounds: LayoutRect,
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

fn decimal<T: core::str::FromStr>(text: &str) -> Option<T> {
    text.bytes()
        .all(|byte| byte.is_ascii_digit())
        .then_some(())?;
    text.parse().ok()
}

pub(super) fn display_time(text: &str) -> Option<(String, &'static str)> {
    let [_, _, b':', _, _, b':', _, _] = *text.as_bytes() else {
        return None;
    };
    let time = LocalTime::new(
        decimal(text.get(..2)?)?,
        decimal(text.get(3..5)?)?,
        decimal(text.get(6..)?)?,
        0,
    )
    .ok()?;
    let hour = (time.hour() + 11) % 12 + 1;
    let period = if time.hour() < 12 { "AM" } else { "PM" };
    Some((format!("{hour}:{:02}", time.minute()), period))
}

pub(super) fn display_date(text: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    // The Unix epoch started on Thursday.
    const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let [_, _, _, _, b'-', _, _, b'-', _, _] = *text.as_bytes() else {
        return None;
    };
    let year: i32 = decimal(text.get(..4)?)?;
    let month = decimal(text.get(5..7)?)?;
    let day = decimal(text.get(8..)?)?;
    // The Gregorian calendar repeats every 400 years. Use a cycle after the
    // epoch so the shared civil conversion also handles earlier dates.
    let date = LocalDate::new(2000 + year % 400, month, day).ok()?;
    let midnight = LocalTime::new(0, 0, 0, 0).ok()?;
    let utc = UtcOffsetSeconds::new(0).ok()?;
    let ticks = LocalDateTime::new(date, midnight)
        .to_offset_instant(utc, TemporalScale::Seconds)
        .ok()?
        .ticks;
    let weekday = WEEKDAYS[((ticks / 86_400) % 7) as usize];
    let month = MONTHS[usize::from(month - 1)];
    Some(format!("{weekday} · {month} {day}"))
}

impl NativeFaceScene {
    /// Prepare the graphical viewer selected by the Host. A Face without a
    /// compatible photograph, clock and weather opens in the usual reader.
    pub fn prepare_picture_frame(
        face: Face,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        expression: &conduit_face::AdmittedVisualExpression,
        preferred_temperature: Option<conduit_core::Unit>,
    ) -> Result<Self, FaceSceneError> {
        let mut scene = Self::prepare_visual_fallback_with_temperature(
            face,
            width,
            height,
            interaction_admitted,
            expression,
            preferred_temperature,
        )?;
        scene.enable_picture_frame(preferred_temperature)?;
        Ok(scene)
    }

    /// Open an admitted photograph as a cover page. All original reading pages,
    /// disclosure and action mappings remain reachable through ordinary paging.
    fn enable_picture_frame(
        &mut self,
        preferred_temperature: Option<conduit_core::Unit>,
    ) -> Result<(), FaceSceneError> {
        if self.picture_frame.is_none()
            && let Some(image) = image::prepare(&self.face)?
            && let Some(prepared) = self.prepare_picture(image, preferred_temperature)
        {
            for row in &mut self.primary {
                row.page += 1;
            }
            self.picture_frame = Some(prepared);
        }
        Ok(())
    }

    fn prepare_picture(
        &self,
        (key, index): (conduit_face::GraphicsImageKey, usize),
        preferred_temperature: Option<conduit_core::Unit>,
    ) -> Option<PicturePage> {
        let face = &self.face;
        let semantic = |name: &str| {
            let mut matches = face.subjects.iter().enumerate().filter(|(_, subject)| {
                matches!(&subject.role, FaceRole::Semantic(kind) if kind.as_str() == name)
            });
            let found = matches.next()?;
            matches.next().is_none().then_some(found)
        };
        let (_, image) = semantic("media/image")?;
        if face.properties.get(index)?.subject != image.identity {
            return None;
        }
        let (_, clock) = semantic("time/civil-observation")?;
        let property = |name: &str| {
            face.properties
                .iter()
                .position(|p| p.subject == clock.identity && p.name == name)
        };
        let date = display_date(property_text(face, property("local-date")?)?)?;
        let (time, period) = display_time(property_text(face, property("local-time")?)?)?;
        let (weather_subject_index, weather_subject) = semantic("weather/conditions")?;
        // The first Body wording is the complete owner or localized weather
        // summary made by document::prepare. Attributions follow it.
        let weather = self.primary.iter().position(|r| {
            r.subject_index == Some(weather_subject_index) && r.role == GraphicsTextRole::Body
        })?;
        let summary = document::weather_summary(face, weather_subject, preferred_temperature)?;
        if self.primary[weather].text != summary {
            return None;
        }
        let texts = [
            date.as_str(),
            time.as_str(),
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
        let gap = 6;
        let label_style = self.typography.style(GraphicsTextRole::Label.into());
        let measure = |text, style| {
            let metrics = crate::display::typography::intrinsic_text_metrics(
                text,
                style,
                self.text_scale_percent,
            )
            .ok()?;
            u16::try_from(metrics.preferred_inline).ok()
        };
        let period_width = measure(period, label_style)?;
        let period_height = u16::try_from(
            TextLayout::new_selected(period, label_style, period_width, self.text_scale_percent)
                .ok()?
                .finish_height(),
        )
        .ok()?;
        let mut widths = [0u16; 3];
        let mut heights = [0u16; 3];
        for (index, (text, role)) in texts.into_iter().zip(ROLES).enumerate() {
            let style = self.typography.style(role.into());
            let measured = measure(text, style)?;
            if index == 1 && measured.checked_add(gap)?.checked_add(period_width)? > usable {
                return None;
            }
            widths[index] = measured.min(usable);
            heights[index] = u16::try_from(
                TextLayout::new_selected(text, style, widths[index], self.text_scale_percent)
                    .ok()?
                    .finish_height(),
            )
            .ok()?;
        }
        let max_line_width = widths[0].max(widths[1] + gap + period_width).max(widths[2]);
        heights[1] = heights[1].max(period_height);
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
        let lines = core::array::from_fn(|index| {
            let h = heights[index];
            let rect = LayoutRect {
                x: (margin + pad) as i16,
                y: cursor as i16,
                width: widths[index],
                height: h,
            };
            cursor += h + gap;
            rect
        });
        let period_bounds = LayoutRect {
            x: lines[1].x + (widths[1] + gap) as i16,
            y: lines[1].y + ((heights[1] - period_height) / 2) as i16,
            width: period_width,
            height: period_height,
        };
        Some(PicturePage {
            image: key,
            date,
            time,
            period,
            period_bounds,
            weather,
            panel,
            lines,
        })
    }

    pub(super) fn picture_page(&self) -> Result<Option<FaceFrame<'_>>, FaceSceneError> {
        let Some(page) = &self.picture_frame else {
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
        push(
            &mut scene,
            GraphicsCommand::image(screen, screen, page.image),
        )?;
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
            page.date.as_str(),
            page.time.as_str(),
            self.primary[page.weather].text.as_str(),
        ];
        for (((text, role), bounds), paint) in texts.into_iter().zip(ROLES).zip(page.lines).zip([
            GraphicsPaintRole::Muted,
            GraphicsPaintRole::Foreground,
            GraphicsPaintRole::Foreground,
        ]) {
            push(
                &mut scene,
                GraphicsCommand::text(bounds, screen, paint, text)
                    .and_then(|command| command.with_text_role(role)),
            )?;
        }
        push(
            &mut scene,
            GraphicsCommand::text(
                page.period_bounds,
                screen,
                GraphicsPaintRole::Foreground,
                page.period,
            )
            .and_then(|command| command.with_text_role(GraphicsTextRole::Label)),
        )?;
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
