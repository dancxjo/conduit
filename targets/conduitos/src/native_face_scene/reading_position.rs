//! Heap-free Mask reading state across replacement of the exact same document.
use super::*;
use sha2::{Digest, Sha256};

/// This is not a Show, action, credential or portable navigation authority.
/// Its private document key includes the exact Face and both measured row sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeReadingPosition {
    document: [u8; 32],
    typography: Typography,
    page: usize,
    details: bool,
}

fn rect(digest: &mut Sha256, value: LayoutRect) {
    digest.update(value.x.to_le_bytes());
    digest.update(value.y.to_le_bytes());
    digest.update(value.width.to_le_bytes());
    digest.update(value.height.to_le_bytes());
}

impl NativeFaceScene {
    fn reading_document_key(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"conduit.native-reading-position@1");
        digest.update(self.face.identity.as_str().len().to_le_bytes());
        digest.update(self.face.identity.as_str().as_bytes());
        digest.update(self.face.revision.to_le_bytes());
        rect(&mut digest, self.screen);
        digest.update(self.text_scale_percent.to_le_bytes());
        digest.update(self.header_top.to_le_bytes());
        digest.update(self.focus_marker_width.to_le_bytes());
        digest.update([
            self.interaction_admitted as u8,
            self.local_input_available as u8,
            self.local_creche_preferences as u8,
        ]);
        if let Some(corners) = self.look_corners {
            digest.update([1, corners.control, corners.panel]);
        } else {
            digest.update([0]);
        }
        for rows in [&self.primary, &self.details] {
            digest.update(rows.len().to_le_bytes());
            for row in rows {
                digest.update(row.page.to_le_bytes());
                rect(&mut digest, row.bounds);
                rect(&mut digest, row.text_bounds());
                digest.update([row.role as u8, row.region_start as u8]);
                digest.update(row.region_inset.to_le_bytes());
                digest.update([row.region.is_some() as u8, row.region.unwrap_or(0)]);
                if let Some(bounds) = row.panel_bounds {
                    digest.update([1]);
                    rect(&mut digest, bounds);
                } else {
                    digest.update([0]);
                }
                digest.update(row.text.len().to_le_bytes());
                digest.update(row.text.as_bytes());
            }
        }
        digest.finalize().into()
    }

    /// Bounded preparation work visits the existing finite document without
    /// allocating or retaining any old Face/Show owners. Drafts are not cursors.
    pub fn reading_position(&self) -> Option<NativeReadingPosition> {
        if self.showing_diagram || self.has_pending_input() || self.local_notice.is_some() {
            return None;
        }
        Some(NativeReadingPosition {
            document: self.reading_document_key(),
            typography: self.typography,
            page: self.page,
            details: self.showing_details,
        })
    }

    /// Restore only after a fresh Scene has been prepared under its own custody.
    /// A changed identity, revision, layout, typography or draft refuses purely.
    pub fn restore_reading_position(&mut self, position: NativeReadingPosition) -> bool {
        if self.showing_diagram
            || self.has_pending_input()
            || self.local_notice.is_some()
            || self.typography != position.typography
            || self.reading_document_key() != position.document
        {
            return false;
        }
        let rows = if position.details {
            &self.details
        } else {
            &self.primary
        };
        let pages = rows.iter().map(|row| row.page + 1).max().unwrap_or(1);
        if position.page >= pages {
            return false;
        }
        self.showing_details = position.details;
        self.page = position.page;
        self.focus = None;
        if let Some(reading) = self.graph_reading.as_mut() {
            reading.active = false;
        }
        true
    }
}
