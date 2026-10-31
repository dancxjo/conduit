//! Fixed spatial composition of the existing report and retained-history Masks.
use conduit_face::{Face, GraphicsScene};

pub use crate::view::Presentation;
pub struct Columns {
    pub report_width: u16,
    pub history_width: u16,
    width: u16,
    height: u16,
    report: Vec<u32>,
    history: Vec<u32>,
}
pub const GAP: u16 = 12;
impl Columns {
    pub fn new(width: u16, height: u16) -> Option<Self> {
        if width < 972 {
            return None;
        }
        Self::at_divider(width, height, (u32::from(width) * 36 / 100) as u16)
    }
    pub fn at_divider(width: u16, height: u16, requested: u16) -> Option<Self> {
        if width < 652 || height < 396 {
            return None;
        }
        let report_width = requested.clamp(320, width - 320 - GAP);
        let history_width = width - report_width - GAP;
        Some(Self {
            report_width,
            history_width,
            width,
            height,
            report: vec![0; usize::from(report_width) * usize::from(height)],
            history: vec![0; usize::from(history_width) * usize::from(height)],
        })
    }
    #[cfg(test)]
    pub fn render(
        &mut self,
        face: Face,
        history: Option<&GraphicsScene>,
        tick: usize,
        preferred: Option<conduit_core::Unit>,
        notices: Notices<'_>,
        pixels: &mut [u32],
    ) -> Result<(), String> {
        self.render_with_palette(
            face,
            history,
            tick,
            Presentation {
                unit: preferred,
                details: false,
                retained_snapshot: false,
                look: None,
            },
            notices,
            pixels,
        )
    }
    pub fn render_with_palette(
        &mut self,
        face: Face,
        history: Option<&GraphicsScene>,
        tick: usize,
        presentation: Presentation<'_>,
        notices: Notices<'_>,
        pixels: &mut [u32],
    ) -> Result<(), String> {
        let preferred = presentation.unit;
        let look = presentation.look;
        if pixels.len() != usize::from(self.width) * usize::from(self.height) {
            return Err("column raster extent mismatch".into());
        }
        crate::view::render_with_details(
            face,
            self.report_width,
            self.height,
            tick,
            &mut self.report,
            Presentation {
                unit: preferred,
                look,
                details: presentation.details,
                retained_snapshot: presentation.retained_snapshot,
            },
        )?;
        if let Some(notice) = notices.report {
            crate::view::render_notice_with_palette(
                notice,
                self.report_width,
                self.height,
                &mut self.report,
                look,
            )?;
        }
        if let Some(scene) = history {
            crate::view::render_graphics_with_palette(
                scene,
                self.history_width,
                self.height,
                &mut self.history,
                look,
            )?;
            crate::view::render_notice_with_palette(
                notices.history,
                self.history_width,
                self.height,
                &mut self.history,
                look,
            )?;
        } else {
            crate::view::render_with_palette(
                crate::view::unavailable("Waiting for retained history"),
                self.history_width,
                self.height,
                0,
                &mut self.history,
                preferred,
                look,
            )?;
        }
        pixels.fill(0x242a32);
        for y in 0..usize::from(self.height) {
            let row = y * usize::from(self.width);
            let left = y * usize::from(self.report_width);
            let right = y * usize::from(self.history_width);
            pixels[row..row + usize::from(self.report_width)]
                .copy_from_slice(&self.report[left..left + usize::from(self.report_width)]);
            let start = row + usize::from(self.report_width + GAP);
            pixels[start..start + usize::from(self.history_width)]
                .copy_from_slice(&self.history[right..right + usize::from(self.history_width)]);
        }
        Ok(())
    }
}
pub struct Notices<'a> {
    pub report: Option<&'a str>,
    pub history: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{GraphicsCommand, GraphicsPaintRole, GraphicsShapeStyle, LayoutRect};
    #[test]
    fn both_panels_remain_visible_and_report_changes_do_not_replace_history() {
        let mut columns = Columns::new(1024, 600).unwrap();
        assert!(columns.history_width >= 640);
        let bounds = LayoutRect {
            x: 0,
            y: 0,
            width: columns.history_width,
            height: 564,
        };
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::rect(
                    bounds,
                    bounds,
                    GraphicsPaintRole::Accent,
                    GraphicsShapeStyle::Fill,
                )
                .unwrap(),
            )
            .unwrap();
        let mut pixels = vec![0; 1024 * 600];
        let mut previous = None;
        for tick in 0..4 {
            columns
                .render(
                    crate::view::unavailable(if tick % 2 == 0 {
                        "Report A"
                    } else {
                        "Report B"
                    }),
                    Some(&scene),
                    tick,
                    None,
                    Notices {
                        report: None,
                        history: "RETAINED HISTORY",
                    },
                    &mut pixels,
                )
                .unwrap();
            let x = usize::from(columns.report_width + GAP);
            let chart: Vec<_> = pixels
                .as_chunks::<1024>()
                .0
                .iter()
                .flat_map(|row| row[x..].iter().copied())
                .collect();
            assert!(chart.iter().any(|v| *v != 0));
            if let Some(previous) = previous.as_ref() {
                assert_eq!(&chart, previous);
            }
            previous = Some(chart);
            assert!(pixels.as_chunks::<1024>().0.iter().any(|row| {
                row[..usize::from(columns.report_width)]
                    .iter()
                    .any(|v| *v != 0)
            }));
        }
    }
    #[test]
    fn narrow_screens_keep_existing_mask_and_bad_buffers_refuse() {
        assert!(Columns::new(800, 600).is_none());
        let mut columns = Columns::new(1024, 600).unwrap();
        assert!(
            columns
                .render(
                    crate::view::unavailable("Waiting"),
                    None,
                    0,
                    None,
                    Notices {
                        report: None,
                        history: ""
                    },
                    &mut []
                )
                .is_err()
        );
    }
}

#[cfg(test)]
mod actual_preview {
    #[test]
    #[ignore = "requires explicitly pinned private Face/history input and output directory"]
    fn actual_report_and_history_fit_one_fixed_screen() {
        let folder = std::path::PathBuf::from(std::env::var_os("CONDUIT_COLUMN_PREVIEW").unwrap());
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(folder.join("current-face.json")).unwrap())
                .unwrap();
        let face: conduit_face::Face = serde_json::from_value(document["face"].clone()).unwrap();
        face.validate().unwrap();
        let original = face.clone();
        let content_height = crate::display_clock::content_height(600).unwrap();
        let content_len = 1024 * usize::from(content_height);
        let mut columns = super::Columns::new(1024, content_height).unwrap();
        let preferred = Some(conduit_core::Unit::Fahrenheit);
        let (_, _, pages) =
            crate::property_mask::frame(&face, columns.report_width, content_height, 0, preferred)
                .expect("actual report fits column");
        assert_eq!(pages, 1, "the current report must not rotate");
        let mut history = crate::history_input::HistoryInput::new_at_size(
            preferred,
            columns.history_width,
            content_height - 36,
        );
        history
            .refresh(&folder.join("retained-history.json"))
            .unwrap();
        let mut pixels = vec![0; 1024 * 600];
        columns
            .render(
                face.clone(),
                history.scene(),
                0,
                preferred,
                super::Notices {
                    report: None,
                    history: history.label().unwrap(),
                },
                &mut pixels[..content_len],
            )
            .unwrap();
        let first = pixels.clone();
        for tick in [1, 2, 8, 16] {
            columns
                .render(
                    face.clone(),
                    history.scene(),
                    tick,
                    preferred,
                    super::Notices {
                        report: None,
                        history: history.label().unwrap(),
                    },
                    &mut pixels[..content_len],
                )
                .unwrap();
            assert_eq!(
                pixels, first,
                "same inputs must stay visible without cycling"
            );
        }
        assert_eq!(face, original);
        crate::display_clock::render(1024, 600, 0, &mut pixels).unwrap();
        let mut ppm = b"P6\n1024 600\n255\n".to_vec();
        for pixel in pixels {
            ppm.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
        }
        std::fs::write(folder.join("preview.ppm"), ppm).unwrap();
    }
}
