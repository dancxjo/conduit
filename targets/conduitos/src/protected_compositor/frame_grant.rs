//! Root-owned bounds for one frame's untrusted returned spans.
//! Acceptance counts bytes/pixels only. It proves neither pixel meaning nor
//! successful scanout, surface ownership, Face/Show identity or input authority.
use conduitos_compositor_core::wire::{FrameSpan, OUTPUT_BYTES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GrantError {
    Sequence,
    Bounds,
    Work,
    Coverage,
    Completed,
}

#[derive(Clone, Copy)]
struct ActiveFrame {
    sequence: u64,
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

    /// Starting a new frame replaces old pending work; frame identities cannot
    /// be reused after acknowledgement, cancellation or a rejected stale begin.
    pub(super) fn begin(&mut self, sequence: u64) -> Result<(), GrantError> {
        if sequence == 0 || sequence <= self.last_sequence || self.maximum_pixels == 0 {
            return Err(GrantError::Sequence);
        }
        self.last_sequence = sequence;
        self.active = Some(ActiveFrame {
            sequence,
            pixels: 0,
            complete: false,
        });
        Ok(())
    }

    /// Admit a single bounded response. A dirty frame need not cover the whole
    /// viewport. Duplicate/overlapping spans still consume the viewport budget.
    pub(super) fn admit(
        &mut self,
        span: FrameSpan,
        requested_sequence: u64,
        maximum_pixels: u32,
    ) -> Result<(), GrantError> {
        let mut frame = self.active.ok_or(GrantError::Sequence)?;
        if requested_sequence != frame.sequence || span.sequence != frame.sequence {
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
            || span.window_offset % 4 != 0
            || span
                .pixels
                .checked_mul(4)
                .and_then(|bytes| span.window_offset.checked_add(bytes))
                .is_none_or(|end| end > OUTPUT_BYTES)
        {
            return Err(GrantError::Bounds);
        }
        let accepted = frame
            .pixels
            .checked_add(u64::from(span.pixels))
            .ok_or(GrantError::Work)?;
        if accepted > self.maximum_pixels {
            return Err(GrantError::Work);
        }
        let coverage = span.coverage.iter().try_fold(0_u64, |sum, &count| {
            sum.checked_add(u64::from(count))
                .ok_or(GrantError::Coverage)
        })?;
        if coverage > accepted || (!span.complete && coverage != 0) {
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
        if frame.sequence != sequence {
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
    fn span(sequence: u64, pixels: u32, complete: bool) -> FrameSpan {
        FrameSpan {
            sequence,
            x: 0,
            y: 0,
            window_offset: 0,
            pixels,
            complete,
            coverage: [0; 8],
        }
    }
    #[test]
    fn forged_coverage_first_pixel_refuses_without_completing_grant() {
        let mut grant = FrameGrant::new(100, 100);
        grant.begin(1).unwrap();
        let mut forged = span(1, 1, true);
        forged.coverage[0] = 10_000;
        assert_eq!(grant.admit(forged, 1, 1), Err(GrantError::Coverage));
        assert_eq!(grant.ack(1), Err(GrantError::Completed));
        grant.admit(span(1, 1, true), 1, 1).unwrap();
        grant.ack(1).unwrap();
    }
    #[test]
    fn repeated_valid_spans_consume_finite_budget() {
        let mut grant = FrameGrant::new(2, 2);
        grant.begin(1).unwrap();
        grant.admit(span(1, 2, false), 1, 2).unwrap();
        grant.admit(span(1, 2, false), 1, 2).unwrap();
        assert_eq!(grant.admit(span(1, 1, true), 1, 2), Err(GrantError::Work));
        assert_eq!(grant.ack(1), Err(GrantError::Completed));
    }
    #[test]
    fn stale_sequence_duplicate_completion_and_reused_begin_refuse() {
        let mut grant = FrameGrant::new(3, 3);
        assert_eq!(grant.begin(0), Err(GrantError::Sequence));
        grant.begin(1).unwrap();
        assert_eq!(
            grant.admit(span(2, 1, true), 1, 1),
            Err(GrantError::Sequence)
        );
        assert_eq!(
            grant.admit(span(1, 1, true), 2, 1),
            Err(GrantError::Sequence)
        );
        grant.admit(span(1, 1, true), 1, 1).unwrap();
        assert_eq!(
            grant.admit(span(1, 1, true), 1, 1),
            Err(GrantError::Completed)
        );
        assert_eq!(grant.ack(2), Err(GrantError::Sequence));
        grant.ack(1).unwrap();
        assert_eq!(grant.ack(1), Err(GrantError::Sequence));
        assert_eq!(grant.begin(1), Err(GrantError::Sequence));
        grant.begin(2).unwrap();
        grant.invalidate();
        assert_eq!(grant.ack(2), Err(GrantError::Sequence));
        assert_eq!(grant.begin(2), Err(GrantError::Sequence));
    }
    #[test]
    fn legitimate_partial_dirty_frame_uses_only_actual_accepted_coverage() {
        let mut grant = FrameGrant::new(100, 100);
        grant.begin(41).unwrap();
        grant.admit(span(41, 5, false), 41, 8).unwrap();
        let mut last = span(41, 3, true);
        last.x = 7;
        last.y = 9;
        last.coverage[0] = 6;
        last.coverage[1] = 2;
        grant.admit(last, 41, 8).unwrap();
        grant.ack(41).unwrap();
    }
    #[test]
    fn zero_oversized_and_wrapped_ranges_refuse() {
        let mut grant = FrameGrant::new(4, 4);
        grant.begin(1).unwrap();
        for bad in [
            span(1, 0, true),
            span(1, 5, true),
            FrameSpan {
                x: u32::MAX,
                ..span(1, 1, true)
            },
            FrameSpan {
                y: 4,
                ..span(1, 1, true)
            },
            FrameSpan {
                window_offset: u32::MAX - 3,
                ..span(1, 1, true)
            },
            FrameSpan {
                window_offset: 1,
                ..span(1, 1, true)
            },
        ] {
            assert_eq!(grant.admit(bad, 1, 4), Err(GrantError::Bounds));
        }
        let mut partial = span(1, 1, false);
        partial.coverage[0] = 1;
        assert_eq!(grant.admit(partial, 1, 4), Err(GrantError::Coverage));
        assert_eq!(
            grant.admit(span(1, 1, true), 1, OUTPUT_BYTES / 4 + 1),
            Err(GrantError::Bounds)
        );
    }
}
