//! Measured Mask chrome, shared by document admission and frame assembly.
use super::FaceSceneError;
use crate::display::{SPACE_SM, SPACE_XL, typography::TextLayout};
use conduit_face::{GraphicsTextRole, LayoutRect};

/// Background, heading, page marker, two separators and footer text.
pub(super) const COMMANDS: usize = 6;
const EDGE: u16 = 8;
pub(super) const HEADER: u16 = 56;
pub(super) const FOOTER: u16 = 56;
const COMPACT_HEADINGS: [&str; 3] = ["VIEW", "DETAILS", "GRAPH"];
const HEADINGS: [&str; 6] = [
    "CURRENT VIEW",
    "DETAILS",
    "PATCHBAY",
    "OWNER FACE / SNAPSHOT",
    "OWNER FACE / DETAILS",
    "OWNER FACE / PATCHBAY",
];
pub(super) const EDITOR_FOOTER: &str =
    "Type message · Backspace erase · Tab switch plot · F2 details";
pub(super) const EDITOR_COMPACT_FOOTER: &str = "Type · F2";
const COMPACT_FOOTERS: [&str; 3] = ["Read only · F2", "PgDn · F2", EDITOR_COMPACT_FOOTER];
const FOOTERS: [&str; 6] = [
    "Read only · PgUp/PgDn pages · F2 facts · F3 Patchbay",
    "Read only · PgUp/PgDn pages · F2 facts",
    "PgUp/PgDn pages · Tab next · F2 return",
    "Tab next · Enter act · F2 inspect · F3 Patchbay",
    "PgUp/PgDn pages · Tab next · Enter act · F2 inspect",
    EDITOR_FOOTER,
];

pub(super) struct Chrome {
    pub heading: LayoutRect,
    pub marker: LayoutRect,
    pub footer: LayoutRect,
    pub top: u16,
    pub bottom: u16,
    pub compact: bool,
    pub heading_role: GraphicsTextRole,
}

fn height(
    text: &str,
    role: GraphicsTextRole,
    width: u16,
    scale: u16,
) -> Result<u16, FaceSceneError> {
    let height = TextLayout::new_scaled(text, role.into(), width, scale)
        .map_err(|_| FaceSceneError::InputBound)?
        .finish_height();
    u16::try_from(height).map_err(|_| FaceSceneError::InvalidExtent)
}

impl Chrome {
    pub fn prepare(screen: LayoutRect, scale: u16) -> Result<Self, FaceSceneError> {
        let width = screen
            .width
            .checked_sub(SPACE_XL * 2)
            .filter(|width| *width > SPACE_SM * 2)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let compact = screen.width < 640 || (screen.width < 1000 && scale > 125);
        let heading_role = if compact {
            GraphicsTextRole::Label
        } else {
            GraphicsTextRole::Heading
        };
        let marker_width = width / 2;
        let heading_width = width - marker_width - SPACE_SM;
        let headings = if compact {
            &COMPACT_HEADINGS[..]
        } else {
            &HEADINGS[..]
        };
        let mut heading_height = 32;
        for heading in headings {
            heading_height =
                heading_height.max(height(heading, heading_role, heading_width, scale)?);
        }
        // A document has at most 32,768 rows, hence no more than 32,768 pages.
        // Reserve the complete largest marker before pagination, avoiding a
        // circular dependency between page count and usable content height.
        let marker_height = height(
            if compact {
                "32768/32768"
            } else {
                "32768 / 32768"
            },
            GraphicsTextRole::Label,
            marker_width,
            scale,
        )?;
        let top = HEADER.max(
            heading_height
                .max(marker_height)
                .checked_add(EDGE * 2)
                .ok_or(FaceSceneError::InvalidExtent)?,
        );
        let mut footer_height = 40;
        let footers = if compact {
            &COMPACT_FOOTERS[..]
        } else {
            &FOOTERS[..]
        };
        for footer in footers {
            footer_height =
                footer_height.max(height(footer, GraphicsTextRole::Muted, width, scale)?);
        }
        let footer_extent = FOOTER.max(
            footer_height
                .checked_add(EDGE * 2)
                .ok_or(FaceSceneError::InvalidExtent)?,
        );
        let bottom = screen
            .height
            .checked_sub(footer_extent)
            .filter(|bottom| *bottom > top)
            .ok_or(FaceSceneError::InvalidExtent)?;
        Ok(Self {
            heading: LayoutRect {
                x: SPACE_XL as i16,
                y: EDGE as i16,
                width: heading_width,
                height: top - EDGE * 2,
            },
            marker: LayoutRect {
                x: (SPACE_XL + heading_width + SPACE_SM) as i16,
                y: EDGE as i16,
                width: marker_width,
                height: top - EDGE * 2,
            },
            footer: LayoutRect {
                x: SPACE_XL as i16,
                y: (bottom + EDGE) as i16,
                width,
                height: footer_extent - EDGE * 2,
            },
            top,
            bottom,
            compact,
            heading_role,
        })
    }

    /// Only the visible provisional suffix is shortened. The argument buffer
    /// remains authoritative for submission; geometry uses the paint metrics.
    pub fn draft_preview(
        &self,
        value: &str,
        scale: u16,
    ) -> Result<alloc::string::String, FaceSceneError> {
        // At most 64 suffix bytes and 130 measured candidates across two hint forms.
        // Never scan or clone the full admitted input to paint its preview.
        let mut initial = value.len().saturating_sub(64);
        while !value.is_char_boundary(initial) {
            initial += 1;
        }
        for compact in [false, true] {
            let mut start = initial;
            loop {
                let preview = alloc::format!(
                    "Draft{}: {}{} · Enter{}",
                    if start > 0 { " (end)" } else { "" },
                    if start > 0 { "…" } else { "" },
                    &value[start..],
                    if compact { "" } else { " applies" }
                );
                if preview.len() <= conduit_face::MAX_GRAPHICS_TEXT_BYTES
                    && self.admit_footer(&preview, scale).is_ok()
                {
                    return Ok(preview);
                }
                let Some(next) = value[start..].chars().next() else {
                    break;
                };
                start += next.len_utf8();
            }
        }
        // Compact chrome can be narrower than the explicit suffix label.
        let preview = alloc::string::String::from("Draft… · Enter");
        self.admit_footer(&preview, scale)?;
        Ok(preview)
    }

    pub fn admit_footer(&self, text: &str, scale: u16) -> Result<(), FaceSceneError> {
        if height(text, GraphicsTextRole::Muted, self.footer.width, scale)? > self.footer.height {
            return Err(FaceSceneError::InputBound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod draft_tests {
    use super::*;
    #[test]
    fn suffix_is_measured_at_each_supported_width_and_scale() {
        for (width, height, scale) in [
            (320, 240, 100),
            (320, 240, 200),
            (640, 480, 100),
            (640, 480, 200),
            (1280, 800, 100),
            (1280, 800, 200),
        ] {
            let chrome = Chrome::prepare(
                LayoutRect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                },
                scale,
            )
            .unwrap();
            for value in ["a".repeat(64), "é".repeat(32), "雪".repeat(21)] {
                let preview = chrome.draft_preview(&value, scale).unwrap();
                chrome.admit_footer(&preview, scale).unwrap();
                assert!(preview.len() <= conduit_face::MAX_GRAPHICS_TEXT_BYTES);
                assert!(preview.contains("Draft") && preview.contains("Enter"));
                if !preview.contains(&value) {
                    assert!(preview.contains('…'));
                }
            }
        }
    }
}
