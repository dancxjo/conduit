//! Actual constant/exact renderer sequence. This is not a kernel queue.
use crate::rendered_gesture_occurrence::{ActualGestureRenderer,PreparedRenderedGestureOccurrence};
use crate::{GreetingRenderCursor,GreetingRenderedFrame,ExactMaterialLinguisticGreetingRenderCursor,ExactMaterialLinguisticGreetingPitchFrame,SpeechGestureRenderRefusal};
/// Complete rendered output with its original optional exact pitch evidence.
/// A uniform inline container avoids additional per-frame heap allocation.
pub struct ActualRenderedFrame {
    rendered: GreetingRenderedFrame,
    pitch: Option<ExactMaterialLinguisticGreetingPitchFrame>,
}
impl ActualRenderedFrame {
    pub fn rendered(&self)->&GreetingRenderedFrame { &self.rendered }
    pub fn pitch(&self)->Option<&ExactMaterialLinguisticGreetingPitchFrame> { self.pitch.as_ref() }
}
enum SegmentCursor<'p,'renderer> {
    Constant(GreetingRenderCursor<'p,'renderer>),
    Exact(ExactMaterialLinguisticGreetingRenderCursor<'p,'renderer>),
}
pub struct PreparedGestureSequence<'p,'a,'renderer,'choice,'joined,'material,'linguistic> {
    occurrences: &'p [PreparedRenderedGestureOccurrence<'a,'renderer,'choice,'joined,'material,'linguistic>],
}
impl<'p,'a,'renderer,'choice,'joined,'material,'linguistic> PreparedGestureSequence<'p,'a,'renderer,'choice,'joined,'material,'linguistic>
where 'material:'choice,'joined:'choice {
    pub fn prepare(occurrences:&'p [PreparedRenderedGestureOccurrence<'a,'renderer,'choice,'joined,'material,'linguistic>])->Result<Self,SpeechGestureRenderRefusal> {
        if occurrences.is_empty() || occurrences.len()>32 { return Err(SpeechGestureRenderRefusal::ResourceBound); }
        let basis=occurrences[0].shared().joined();let hz=occurrences[0].sample_rate_hz();
        // This sequence profile is strictly adjacent; overlap requires an
        // independently admitted compositor rather than implicit concatenation.
        for pair in occurrences.windows(2) {
            if pair[0].frame_range().end!=pair[1].frame_range().start { return Err(SpeechGestureRenderRefusal::ForeignBasis); }
        }
        for o in occurrences { if o.sample_rate_hz()!=hz || !core::ptr::eq(o.shared().joined(),basis) {return Err(SpeechGestureRenderRefusal::ForeignBasis);} }
        Ok(Self{occurrences})
    }
    pub fn occurrences(&self)->&'p [PreparedRenderedGestureOccurrence<'a,'renderer,'choice,'joined,'material,'linguistic>] { self.occurrences }
    pub fn cursor(&'p self)->GestureSequenceCursor<'p,'a,'renderer,'choice,'joined,'material,'linguistic> {
        GestureSequenceCursor{owner:self,index:0,active:None}
    }
    pub fn next(&'p self,cursor:&mut GestureSequenceCursor<'p,'a,'renderer,'choice,'joined,'material,'linguistic>)->Result<Option<ActualRenderedFrame>,SpeechGestureRenderRefusal> {
        if !core::ptr::eq(cursor.owner,self) {return Err(SpeechGestureRenderRefusal::ForeignBasis);}
        while let Some(occurrence)=self.occurrences.get(cursor.index) {
            if cursor.active.is_none() {cursor.active=Some(match occurrence.renderer() {
                ActualGestureRenderer::Constant(r)=>SegmentCursor::Constant(r.cursor()),
                ActualGestureRenderer::Exact(r)=>SegmentCursor::Exact(r.cursor()),
            });}
            let rendered=match (occurrence.renderer(),cursor.active.as_mut().unwrap()) {
                (ActualGestureRenderer::Constant(r),SegmentCursor::Constant(c))=>r.next(c)?.map(|rendered|ActualRenderedFrame{rendered,pitch:None}),
                (ActualGestureRenderer::Exact(r),SegmentCursor::Exact(c))=>r.next(c)?.map(|frame|{let (pitch,rendered)=frame.into_parts();ActualRenderedFrame{rendered,pitch:Some(pitch)}}),
                _=>return Err(SpeechGestureRenderRefusal::ForeignBasis),
            };
            if rendered.is_some(){return Ok(rendered);}
            cursor.active=None;cursor.index+=1;
        }
        Ok(None)
    }
}
pub struct GestureSequenceCursor<'p,'a,'renderer,'choice,'joined,'material,'linguistic> {
    owner:&'p PreparedGestureSequence<'p,'a,'renderer,'choice,'joined,'material,'linguistic>,
    index:usize,
    active:Option<SegmentCursor<'p,'renderer>>,
}
