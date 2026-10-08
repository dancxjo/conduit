//! Exact admitted linguistic cycle trajectories on the original shared IPA path.
//! This retains preparation and frame evidence; it grants no playback authority.
use crate::{
    common_acoustic_quantities::{execute, SpeechCommonAcousticExecution},
    greeting_programs::{PITCH_FRACTION, PITCH_GRID, PITCH_Q8},
    ipa_gestures::PreparedIpaContextualGreetingPhoneGestures,
    pitch_trajectory::PreparedLinguisticPitch,
    semantic::*,
    *,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeRustBinding;

pub struct PreparedLinguisticGreetingRenderer<
    'a,
    'renderer,
    'choice,
    'joined,
    'material,
    'linguistic,
> {
    shared: &'a PreparedIpaContextualGreetingPhoneGestures<'choice, 'joined, 'material>,
    renderer: &'a PreparedGreetingRenderer<'renderer>,
    pitch: &'a PreparedLinguisticPitch<'linguistic>,
    endpoint_checks: Vec<LinguisticGreetingPitchFrame>,
}
pub struct LinguisticGreetingPitchFrame {
    admission: SpeechGesturePitchQ8Request,
    executions: Vec<SpeechCommonAcousticExecution>,
}
impl LinguisticGreetingPitchFrame {
    pub fn admission(&self) -> &SpeechGesturePitchQ8Request {
        &self.admission
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
}
pub struct LinguisticGreetingRenderedFrame {
    pitch: LinguisticGreetingPitchFrame,
    rendered: GreetingRenderedFrame,
}
impl LinguisticGreetingRenderedFrame {
    pub fn pitch(&self) -> &LinguisticGreetingPitchFrame {
        &self.pitch
    }
    pub fn rendered(&self) -> &GreetingRenderedFrame {
        &self.rendered
    }
}
pub struct LinguisticGreetingRenderCursor<'p, 'renderer> {
    owner: *const (),
    inner: GreetingRenderCursor<'p, 'renderer>,
}
impl<'a, 'renderer, 'choice, 'joined, 'material, 'linguistic>
    PreparedLinguisticGreetingRenderer<'a, 'renderer, 'choice, 'joined, 'material, 'linguistic>
where
    'material: 'choice,
    'joined: 'choice,
{
    pub fn shared(
        &self,
    ) -> &'a PreparedIpaContextualGreetingPhoneGestures<'choice, 'joined, 'material> {
        self.shared
    }
    pub fn renderer(&self) -> &'a PreparedGreetingRenderer<'renderer> {
        self.renderer
    }
    pub fn pitch(&self) -> &'a PreparedLinguisticPitch<'linguistic> {
        self.pitch
    }
    pub fn endpoint_checks(&self) -> &[LinguisticGreetingPitchFrame] {
        &self.endpoint_checks
    }
    pub fn cursor(&self) -> LinguisticGreetingRenderCursor<'_, 'renderer> {
        LinguisticGreetingRenderCursor {
            owner: self as *const _ as *const (),
            inner: self.renderer.cursor(),
        }
    }
    pub fn next<'p>(
        &'p self,
        cursor: &mut LinguisticGreetingRenderCursor<'p, 'renderer>,
    ) -> Result<Option<LinguisticGreetingRenderedFrame>, SpeechGestureRenderRefusal> {
        if cursor.owner != self as *const _ as *const () {
            return Err(SpeechGestureRenderRefusal::ForeignBasis);
        }
        let frame = cursor.inner.frame_number();
        if frame == self.renderer.first_target().frame_range().end {
            return Ok(None);
        }
        let pitch = self.at_frame(frame)?;
        let period = i32::try_from(*pitch.admission.projected().whole_q8())
            .map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?;
        Ok(self
            .renderer
            .next_with_period(&mut cursor.inner, Some(period))?
            .map(|rendered| LinguisticGreetingRenderedFrame { pitch, rendered }))
    }
    fn at_frame(
        &self,
        frame: i32,
    ) -> Result<LinguisticGreetingPitchFrame, SpeechGestureRenderRefusal> {
        let range = self.renderer.first_target().frame_range();
        let local = frame
            .checked_sub(range.start)
            .ok_or(SpeechGestureRenderRefusal::ForeignBasis)?;
        let pitch = SpeechPitchAtFrameRequest::new(
            self.pitch.accepted().pitch().clone(),
            u64::try_from(local).map_err(|_| SpeechGestureRenderRefusal::ForeignBasis)?,
            *self.renderer.first_target().basis().sample_rate_hz(),
        )?;
        let original = SpeechGesturePitchFrameRequest::new(
            u64::try_from(frame).map_err(|_| SpeechGestureRenderRefusal::ForeignBasis)?,
            pitch,
            self.shared.contextual().lowered().original_timing().clone(),
        )?;
        let mut executions = Vec::new();
        let input = SpeechPitchProjectionInput::decode(&execute(
            PITCH_GRID,
            original.clone(),
            &mut executions,
        )?)?;
        let fraction =
            SpeechPitchCycleFraction::decode(&execute(PITCH_FRACTION, input, &mut executions)?)?;
        let cycle = SpeechFundamentalCycle::new(*fraction.denominator(), *fraction.numerator())?;
        let request = SpeechCycleAtRateRequest::new(cycle, *original.pitch().sample_rate_hz())?;
        let q8 = SpeechGesturePitchQ8Value::decode(&execute(
            PITCH_Q8,
            request.clone(),
            &mut executions,
        )?)?;
        let projected =
            SpeechCycleQ8AtRate::new(*q8.remainder_numerator(), request, *q8.whole_q8())?;
        let admission = SpeechGesturePitchQ8Request::new(fraction, original, projected)?;
        Ok(LinguisticGreetingPitchFrame {
            admission,
            executions,
        })
    }
}
pub fn prepare_linguistic_greeting_renderer<
    'a,
    'renderer,
    'choice,
    'joined,
    'material,
    'linguistic,
>(
    shared: &'a PreparedIpaContextualGreetingPhoneGestures<'choice, 'joined, 'material>,
    renderer: &'a PreparedGreetingRenderer<'renderer>,
    pitch: &'a PreparedLinguisticPitch<'linguistic>,
) -> Result<
    PreparedLinguisticGreetingRenderer<'a, 'renderer, 'choice, 'joined, 'material, 'linguistic>,
    SpeechGestureRenderRefusal,
>
where
    'material: 'choice,
    'joined: 'choice,
{
    if !core::ptr::eq(renderer.original(), shared.contextual().profile()) {
        return Err(SpeechGestureRenderRefusal::ForeignBasis);
    }
    let segment = shared.contextual().choice().occurrence().segment();
    let exact = SpeechPlannedSegmentIntent::new(
        segment.occurrence().clone(),
        segment.phone().clone(),
        segment.phoneme().clone(),
        segment.prosody().clone(),
        segment.provenance().clone(),
        segment.sources().clone(),
        segment.stress().clone(),
        segment.word_position().clone(),
    )?;
    if &exact != pitch.accepted().pitch().segment() {
        return Err(SpeechGestureRenderRefusal::ForeignBasis);
    }
    let range = renderer.first_target().frame_range();
    let count = range
        .end
        .checked_sub(range.start)
        .ok_or(SpeechGestureRenderRefusal::ResourceBound)?;
    if count <= 0
        || u64::try_from(count).map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?
            > crate::MAXIMUM_UTTERANCE_FRAMES
    {
        return Err(SpeechGestureRenderRefusal::ResourceBound);
    }
    let mut prepared = PreparedLinguisticGreetingRenderer {
        shared,
        renderer,
        pitch,
        endpoint_checks: Vec::new(),
    };
    prepared
        .endpoint_checks
        .push(prepared.at_frame(range.start)?);
    prepared
        .endpoint_checks
        .push(prepared.at_frame(range.end - 1)?);
    Ok(prepared)
}
