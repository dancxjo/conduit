//! Root-owned bounds for one frame's untrusted returned spans.
//! Acceptance verifies declared pixel work and exact ordered completion only. It proves neither pixel meaning nor
//! successful scanout, surface ownership, Face/Show identity or input authority.
use conduitos_compositor_core::{
    FramePlan, FrameRect,
    wire::{FrameSpan, OUTPUT_BYTES},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GrantError {
    Sequence,
    Bounds,
    Work,
    Coverage,
    Completed,
    Order,
}

#[derive(Clone, Copy)]
struct ActiveFrame {
    plan: FramePlan,
    total: u64,
    index: usize,
    x: u32,
    y: u32,
    pixels: u64,
    complete: bool,
}

pub(super) struct FrameGrant {
    width: u32,
    height: u32,
    maximum_pixels: u64,
    last_sequence: u64,
    active: Option<ActiveFrame>,
}

impl FrameGrant {
    pub(super) fn new(width: u16, height: u16) -> Self {
        Self {
            width: u32::from(width),
            height: u32::from(height),
            maximum_pixels: u64::from(width) * u64::from(height),
            last_sequence: 0,
            active: None,
        }
    }

    /// Validate the complete untrusted work descriptor before admitting spans.
    pub(super) fn begin(&mut self, plan: FramePlan) -> Result<(), GrantError> {
        if plan.sequence == 0 || plan.sequence <= self.last_sequence || self.maximum_pixels == 0 {
            return Err(GrantError::Sequence);
        }
        let count = usize::from(plan.count);
        if count == 0 || count > plan.rects.len() {
            return Err(GrantError::Bounds);
        }
        let mut total = 0u64;
        for (i, rect) in plan.rects.iter().enumerate() {
            if i >= count {
                if *rect != FrameRect::default() {
                    return Err(GrantError::Bounds);
                }
                continue;
            }
            if rect.width == 0
                || rect.height == 0
                || rect
                    .x
                    .checked_add(rect.width)
                    .is_none_or(|end| end > self.width)
                || rect
                    .y
                    .checked_add(rect.height)
                    .is_none_or(|end| end > self.height)
            {
                return Err(GrantError::Bounds);
            }
            for prior in &plan.rects[..i] {
                if rect.x < prior.x + prior.width
                    && prior.x < rect.x + rect.width
                    && rect.y < prior.y + prior.height
                    && prior.y < rect.y + rect.height
                {
                    return Err(GrantError::Work);
                }
            }
            total = total
                .checked_add(u64::from(rect.width) * u64::from(rect.height))
                .ok_or(GrantError::Work)?;
            if total > self.maximum_pixels {
                return Err(GrantError::Work);
            }
        }
        self.last_sequence = plan.sequence;
        self.active = Some(ActiveFrame {
            plan,
            total,
            index: 0,
            x: plan.rects[0].x,
            y: plan.rects[0].y,
            pixels: 0,
            complete: false,
        });
        Ok(())
    }

    /// Every response must continue exactly at the next declared row position.
    pub(super) fn admit(
        &mut self,
        span: FrameSpan,
        requested_sequence: u64,
        maximum_pixels: u32,
    ) -> Result<(), GrantError> {
        let mut frame = self.active.ok_or(GrantError::Sequence)?;
        if requested_sequence != frame.plan.sequence || span.sequence != frame.plan.sequence {
            return Err(GrantError::Sequence);
        }
        if frame.complete {
            return Err(GrantError::Completed);
        }
        if span.pixels == 0
            || maximum_pixels == 0
            || maximum_pixels > OUTPUT_BYTES / 4
            || span.pixels > maximum_pixels
            || span.y >= self.height
            || span
                .x
                .checked_add(span.pixels)
                .is_none_or(|end| end > self.width)
            || !span.window_offset.is_multiple_of(4)
            || span
                .pixels
                .checked_mul(4)
                .and_then(|bytes| span.window_offset.checked_add(bytes))
                .is_none_or(|end| end > OUTPUT_BYTES)
        {
            return Err(GrantError::Bounds);
        }
        let rect = frame.plan.rects[frame.index];
        if span.x != frame.x || span.y != frame.y || span.pixels > rect.x + rect.width - frame.x {
            return Err(GrantError::Order);
        }
        let accepted = frame
            .pixels
            .checked_add(u64::from(span.pixels))
            .ok_or(GrantError::Work)?;
        if accepted > frame.total {
            return Err(GrantError::Work);
        }
        frame.x += span.pixels;
        if frame.x == rect.x + rect.width {
            frame.x = rect.x;
            frame.y += 1;
            if frame.y == rect.y + rect.height {
                frame.index += 1;
                if frame.index < usize::from(frame.plan.count) {
                    frame.x = frame.plan.rects[frame.index].x;
                    frame.y = frame.plan.rects[frame.index].y;
                }
            }
        }
        let at_end = frame.index == usize::from(frame.plan.count);
        if span.complete != at_end || (at_end && accepted != frame.total) {
            return Err(GrantError::Completed);
        }
        let mut coverage = 0u64;
        for (i, &count) in span.coverage.iter().enumerate() {
            if count > 0 && span.selected_mask & (1 << i) == 0 {
                return Err(GrantError::Coverage);
            }
            coverage = coverage
                .checked_add(u64::from(count))
                .ok_or(GrantError::Coverage)?;
        }
        if coverage > accepted
            || u64::from(span.selected_mask.count_ones()) > accepted
            || (!span.complete && (coverage != 0 || span.selected_mask != 0))
        {
            return Err(GrantError::Coverage);
        }
        frame.pixels = accepted;
        frame.complete = span.complete;
        self.active = Some(frame);
        Ok(())
    }

    /// Only a complete returned frame may reach an external scanout owner.
    /// Calling this is NOT a ScanoutAcknowledgement: the caller must separately
    /// validate surface bindings and successfully copy every accepted span.
    pub(super) fn ack(&mut self, sequence: u64) -> Result<(), GrantError> {
        let frame = self.active.ok_or(GrantError::Sequence)?;
        if frame.plan.sequence != sequence {
            return Err(GrantError::Sequence);
        }
        if !frame.complete {
            return Err(GrantError::Completed);
        }
        self.active = None;
        Ok(())
    }

    pub(super) fn invalidate(&mut self) {
        self.active = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan(sequence: u64, rects: &[FrameRect]) -> FramePlan {
        let mut p = FramePlan {
            sequence,
            count: rects.len() as u8,
            rects: [FrameRect::default(); 16],
            conservative_fallback: false,
            cursor_visible: false,
            focus_visible: false,
        };
        p.rects[..rects.len()].copy_from_slice(rects);
        p
    }
    fn rect(x: u32, y: u32, width: u32, height: u32) -> FrameRect {
        FrameRect {
            x,
            y,
            width,
            height,
        }
    }
    fn span(sequence: u64, x: u32, y: u32, pixels: u32, complete: bool) -> FrameSpan {
        FrameSpan {
            sequence,
            x,
            y,
            pixels,
            complete,
            window_offset: 0,
            selected_mask: 0,
            coverage: [0; 8],
        }
    }
    #[test]
    fn descriptor_refuses_empty_overlapping_outside_unused_and_overflow_before_effects() {
        let good = plan(1, &[rect(0, 0, 2, 2)]);
        let mut grant = FrameGrant::new(4, 4);
        let mut zero = good;
        zero.count = 0;
        let mut unused = good;
        unused.rects[15] = rect(0, 0, 1, 1);
        for bad in [
            zero,
            unused,
            plan(1, &[rect(0, 0, 0, 1)]),
            plan(1, &[rect(0, 0, 2, 2), rect(1, 1, 2, 2)]),
            plan(1, &[rect(3, 3, 2, 1)]),
            plan(1, &[rect(u32::MAX, 0, 2, 1)]),
        ] {
            assert!(grant.begin(bad).is_err());
            assert!(grant.active.is_none());
            assert_eq!(grant.last_sequence, 0);
        }
        grant.begin(good).unwrap();
    }
    #[test]
    fn exact_rectangles_rows_and_completion_are_required() {
        let mut grant = FrameGrant::new(8, 8);
        grant
            .begin(plan(7, &[rect(1, 2, 3, 2), rect(6, 7, 2, 1)]))
            .unwrap();
        for bad in [
            span(7, 2, 2, 1, false),
            span(7, 1, 3, 1, false),
            span(7, 1, 2, 4, false),
        ] {
            assert_eq!(grant.admit(bad, 7, 8), Err(GrantError::Order));
        }
        assert_eq!(
            grant.admit(span(7, 1, 2, 1, true), 7, 8),
            Err(GrantError::Completed)
        );
        grant.admit(span(7, 1, 2, 1, false), 7, 8).unwrap();
        assert_eq!(
            grant.admit(span(7, 1, 2, 1, false), 7, 8),
            Err(GrantError::Order)
        );
        grant.admit(span(7, 2, 2, 2, false), 7, 8).unwrap();
        grant.admit(span(7, 1, 3, 3, false), 7, 8).unwrap();
        assert_eq!(grant.ack(7), Err(GrantError::Completed));
        assert_eq!(
            grant.admit(span(7, 6, 7, 2, false), 7, 8),
            Err(GrantError::Completed)
        );
        grant.admit(span(7, 6, 7, 2, true), 7, 8).unwrap();
        assert_eq!(
            grant.admit(span(7, 6, 7, 2, true), 7, 8),
            Err(GrantError::Completed)
        );
        grant.ack(7).unwrap();
        assert_eq!(
            grant.begin(plan(7, &[rect(0, 0, 1, 1)])),
            Err(GrantError::Sequence)
        );
    }
    #[test]
    fn coverage_and_selection_are_bounded_but_fully_obscured_selection_is_valid() {
        let mut grant = FrameGrant::new(8, 8);
        grant.begin(plan(1, &[rect(0, 0, 2, 1)])).unwrap();
        let mut forged = span(1, 0, 0, 1, false);
        forged.selected_mask = 1;
        assert_eq!(grant.admit(forged, 1, 8), Err(GrantError::Coverage));
        grant.admit(span(1, 0, 0, 1, false), 1, 8).unwrap();
        let mut last = span(1, 1, 0, 1, true);
        last.coverage[0] = 3;
        last.selected_mask = 1;
        assert_eq!(grant.admit(last, 1, 8), Err(GrantError::Coverage));
        last.coverage[0] = 1;
        last.selected_mask = 0;
        assert_eq!(grant.admit(last, 1, 8), Err(GrantError::Coverage));
        last.coverage[0] = 0;
        last.selected_mask = 0xff;
        assert_eq!(grant.admit(last, 1, 8), Err(GrantError::Coverage));
        last.selected_mask = 1;
        grant.admit(last, 1, 8).unwrap();
        grant.ack(1).unwrap();
    }
    #[test]
    fn stale_sequence_and_invalid_window_do_not_advance_progress() {
        let mut grant = FrameGrant::new(2, 2);
        grant.begin(plan(1, &[rect(0, 0, 1, 1)])).unwrap();
        assert_eq!(
            grant.admit(span(2, 0, 0, 1, true), 1, 1),
            Err(GrantError::Sequence)
        );
        assert_eq!(
            grant.admit(span(1, 0, 0, 1, true), 2, 1),
            Err(GrantError::Sequence)
        );
        for bad in [
            span(1, 0, 0, 0, true),
            FrameSpan {
                window_offset: 1,
                ..span(1, 0, 0, 1, true)
            },
            FrameSpan {
                window_offset: OUTPUT_BYTES,
                ..span(1, 0, 0, 1, true)
            },
        ] {
            assert_eq!(grant.admit(bad, 1, 1), Err(GrantError::Bounds));
        }
        grant.admit(span(1, 0, 0, 1, true), 1, 1).unwrap();
        assert_eq!(grant.ack(2), Err(GrantError::Sequence));
        grant.invalidate();
        assert_eq!(grant.ack(1), Err(GrantError::Sequence));
    }
}
