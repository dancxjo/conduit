//! Passive reading continuity, resolved against a newly admitted exact Scene.
//! A fingerprint is a local reading wish, never a Show or action authority.
use super::*;
use core::fmt::{self, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativePassiveReadingPosition {
    document: [u8; 32],
    typography: Typography,
    page: usize,
    details: bool,
    focus: Option<[u8; 32]>,
}
struct HashWriter<'a>(&'a mut Sha256);
impl Write for HashWriter<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0.update(text.as_bytes());
        Ok(())
    }
}
// These private, ephemeral fingerprints are compared only within one native
// executable. Streaming Debug of these derived finite semantic records retains
// no text and allocates no formatting buffer; this is not a wire identity.
fn semantic(digest: &mut Sha256, value: &impl fmt::Debug) {
    let _ = write!(HashWriter(digest), "{value:?}\0");
}
fn control_key(scene: &NativeFaceScene, control: FaceControl) -> Option<[u8; 32]> {
    let (action, argument) = scene
        .resolve(&scene.face.identity, scene.face.revision, control)
        .ok()?;
    let mut digest = Sha256::new();
    digest.update(b"native-passive-control@1");
    semantic(&mut digest, action);
    semantic(&mut digest, &argument);
    Some(digest.finalize().into())
}
impl NativeFaceScene {
    fn passive_cursor_ready(&self) -> bool {
        !self.showing_diagram
            && !self.graph_reading.as_ref().is_some_and(|r| r.active)
            && !self.has_pending_input()
            && self.input.pointer.is_none()
            && self.local_notice.is_none()
    }
    fn passive_document_key(&self) -> Option<[u8; 32]> {
        // Shared Body Faces retain Plan/Play provenance in their canonical
        // contribution properties, rather than inventing one outer producer.
        let identity = |name: &str| {
            self.face.properties.iter().any(|p|p.name==name && matches!(&p.value,conduit_face::FacePropertyValue::Identity(value) if !value.is_empty()))
        };
        if self.face.basis.body_id.is_none()
            || !(self.face.basis.plan_id.is_some() || identity("plan-id"))
            || !(self.face.basis.active_play_id.is_some() || identity("active-play-id"))
        {
            return None;
        }
        let mut digest = Sha256::new();
        digest.update(b"native-passive-reading@1");
        let basis = &self.face.basis;
        semantic(
            &mut digest,
            &(
                &basis.body_id,
                &basis.wake_id,
                &basis.source_document_id,
                &basis.checked_plot_id,
                &basis.expanded_plot_id,
                &basis.plan_id,
                &basis.active_play_id,
            ),
        );
        semantic(&mut digest, &self.face.interaction_context);
        for subject in &self.face.subjects {
            semantic(&mut digest, &(&subject.identity, &subject.role));
        }
        semantic(&mut digest, &self.face.relationships);
        semantic(&mut digest, &self.face.composition);
        semantic(&mut digest, &self.face.disclosures);
        // Exact action contracts, targets, names and current availability must
        // all remain stable. Passive data values cannot supply action identity.
        semantic(&mut digest, &self.face.actions);
        for property in &self.face.properties {
            if let conduit_face::FacePropertyValue::Identity(value) = &property.value {
                semantic(&mut digest, &(&property.subject, &property.name, value));
            }
        }
        rect(&mut digest, self.screen);
        digest.update(self.text_scale_percent.to_le_bytes());
        digest.update(self.header_top.to_le_bytes());
        digest.update(self.focus_marker_width.to_le_bytes());
        semantic(
            &mut digest,
            &self.look_corners.map(|c| (c.control, c.panel)),
        );
        digest.update([
            self.interaction_admitted as u8,
            self.local_input_available as u8,
            self.local_creche_preferences as u8,
        ]);
        // Text and image changes may reflow rows. A focused reading wish is
        // resolved onto the fresh control's current page, never old geometry.
        Some(digest.finalize().into())
    }
    /// Fixed inline state only. Drafts, gestures, graph reading and notices are
    /// excluded; none can be silently moved onto a replacement observation.
    pub fn passive_reading_position(&self) -> Option<NativePassiveReadingPosition> {
        if !self.passive_cursor_ready() {
            return None;
        }
        let focus = match self.focus {
            Some(control) => Some(control_key(self, control)?),
            None => None,
        };
        Some(NativePassiveReadingPosition {
            document: self.passive_document_key()?,
            typography: self.typography,
            page: self.page,
            details: self.showing_details,
            focus,
        })
    }
    /// Resolve only under the fresh Scene's current semantic controls. The
    /// caller must obtain/acknowledge a new Show before any interaction occurs.
    pub fn restore_passive_reading_position(
        &mut self,
        position: NativePassiveReadingPosition,
    ) -> bool {
        if !self.passive_cursor_ready()
            || self.typography != position.typography
            || self.passive_document_key() != Some(position.document)
        {
            return false;
        }
        let rows = if position.details {
            &self.details
        } else {
            &self.primary
        };
        let (focus, page) = if let Some(key) = position.focus {
            let Some((control, page)) = rows
                .iter()
                .filter_map(|row| self.eligible(row))
                .find(|(control, _)| control_key(self, *control) == Some(key))
            else {
                return false;
            };
            (Some(control), page)
        } else {
            if position.page >= rows.iter().map(|row| row.page + 1).max().unwrap_or(1) {
                return false;
            }
            (None, position.page)
        };
        self.showing_details = position.details;
        self.page = page;
        self.focus = focus;
        if let Some(reading) = self.graph_reading.as_mut() {
            reading.active = false;
        }
        true
    }
}

#[cfg(test)]
#[path = "passive/tests.rs"]
mod tests;
