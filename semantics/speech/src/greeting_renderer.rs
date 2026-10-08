//! Source-selected explicit temporal targets, with retained declared losses.
use crate::{
    common_acoustic_quantities::{boolean, SpeechCommonAcousticExecution},
    gesture_renderer::{prepare_speech_gesture_renderer_at_time, transfer_greeting_cursor},
    greeting_programs::SECOND,
    semantic::*,
    *,
};
use alloc::vec::Vec;
use conduit_audio::*;
use conduit_plot::rust_binding::NativeRustBinding;
pub struct PreparedGreetingRenderer<'a> {
    original: &'a PreparedGreetingPhoneGestures,
    first: PreparedSpeechGestureRenderer<'a>,
    second: Option<PreparedSpeechGestureRenderer<'a>>,
    boundary: Option<AudioSampleProjectionReceipt>,
}
impl<'a> PreparedGreetingRenderer<'a> {
    pub fn original(&self) -> &PreparedGreetingPhoneGestures {
        self.original
    }
    pub fn first_target(&self) -> &PreparedSpeechGestureRenderer<'a> {
        &self.first
    }
    pub fn second_target(&self) -> Option<&PreparedSpeechGestureRenderer<'a>> {
        self.second.as_ref()
    }
    pub fn boundary_projection(&self) -> Option<&AudioSampleProjectionReceipt> {
        self.boundary.as_ref()
    }
    pub fn cursor(&self) -> GreetingRenderCursor<'_, 'a> {
        GreetingRenderCursor {
            owner: self,
            inner: self.first.cursor(),
            second: false,
        }
    }
    pub fn next<'p>(
        &'p self,
        cursor: &mut GreetingRenderCursor<'p, 'a>,
    ) -> Result<Option<GreetingRenderedFrame>, SpeechGestureRenderRefusal> {
        self.next_with_period(cursor, None)
    }
    pub(crate) fn next_with_period<'p>(
        &'p self,
        cursor: &mut GreetingRenderCursor<'p, 'a>,
        period: Option<i32>,
    ) -> Result<Option<GreetingRenderedFrame>, SpeechGestureRenderRefusal> {
        if !core::ptr::eq(self, cursor.owner) {
            return Err(SpeechGestureRenderRefusal::ForeignBasis);
        }
        let frame = cursor.inner.frame_number();
        let range = self.first.frame_range();
        if frame == range.end {
            return Ok(None);
        }
        let boundary = self
            .boundary
            .as_ref()
            .map(|r| i32::try_from(*r.result().raw().whole_frames()))
            .transpose()
            .map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?
            .unwrap_or(range.end);
        let query = SpeechGreetingTargetQuery::new(
            boundary,
            range.end,
            range.start,
            frame,
            self.second.is_some(),
        )?;
        let mut executions = Vec::new();
        let second = boolean(SECOND, query, &mut executions)?;
        if second && !cursor.second {
            cursor.inner = transfer_greeting_cursor(
                &cursor.inner,
                self.second
                    .as_ref()
                    .ok_or(SpeechGestureRenderRefusal::UnsupportedProfile)?,
                &mut executions,
            )?;
            cursor.second = true;
        }
        let target = if second {
            self.second
                .as_ref()
                .ok_or(SpeechGestureRenderRefusal::UnsupportedProfile)?
        } else {
            &self.first
        };
        let rendered = if let Some(period) = period {
            target.next_with_period(&mut cursor.inner, period)?
        } else {
            target.next(&mut cursor.inner)?
        };
        Ok(rendered.map(|rendered| GreetingRenderedFrame {
            rendered,
            second,
            executions,
        }))
    }
}
pub struct GreetingRenderCursor<'p, 's> {
    owner: &'p PreparedGreetingRenderer<'s>,
    inner: SpeechGestureRenderCursor<'p, 's>,
    second: bool,
}
impl GreetingRenderCursor<'_, '_> {
    pub(crate) fn frame_number(&self) -> i32 {
        self.inner.frame_number()
    }
}
pub struct GreetingRenderedFrame {
    rendered: SpeechGestureRenderedFrame,
    second: bool,
    executions: Vec<SpeechCommonAcousticExecution>,
}
impl GreetingRenderedFrame {
    pub fn rendered(&self) -> &SpeechGestureRenderedFrame {
        &self.rendered
    }
    pub fn is_second_target(&self) -> bool {
        self.second
    }
    pub fn target_selection_and_reset_executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
}
pub fn prepare_greeting_renderer<'a>(
    original: &'a PreparedGreetingPhoneGestures,
    basis_frame: &[u8],
    cycle_frame: &[u8],
) -> Result<PreparedGreetingRenderer<'a>, SpeechGestureRenderRefusal> {
    let first = prepare_speech_gesture_renderer_at_time(
        original.lowered(),
        basis_frame,
        cycle_frame,
        original.lowered().original_timing().nominal_start(),
    )?;
    let (second, boundary) = if let Some(time) = original.second_start() {
        let second = prepare_speech_gesture_renderer_at_time(
            original.lowered(),
            basis_frame,
            cycle_frame,
            time,
        )?;
        let request = AudioSampleProjectionRequest::new(
            first.basis().clone(),
            AudioSampleProjectionQuantity::duration(
                *time.denominator(),
                *time.numerator_seconds(),
            )?,
        )?;
        let boundary = PreparedAudioSampleProjection::new()?.project(&request.encode()?)?;
        // Source admits a nonempty pair before any rendering. No collapsed
        // quantized segment is reset, clamped or silently discarded.
        let range = first.frame_range();
        let point = i32::try_from(*boundary.result().raw().whole_frames())
            .map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?;
        SpeechGreetingTargetQuery::new(point, range.end, range.start, range.start, true)?;
        (Some(second), Some(boundary))
    } else {
        (None, None)
    };
    Ok(PreparedGreetingRenderer {
        original,
        first,
        second,
        boundary,
    })
}
