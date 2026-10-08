//! Borrow actual IPA gesture and renderer owners with their admitted frame range.
//! This is preparation custody, not queued or played evidence.
use crate::{
    PreparedExactMaterialLinguisticGreetingRenderer,
    ipa_gestures::PreparedIpaContextualGreetingPhoneGestures,
    semantic::{SpeechCycleSpecification, SpeechUtteranceIntent},
    PreparedGreetingRenderer,
};
#[derive(Debug, PartialEq, Eq)]
pub enum RenderedOccurrenceRefusal { ForeignGesture, ForeignIntent, Cycle, Range, Clock }
pub enum ActualGestureRenderer<'a,'renderer,'choice,'joined,'material,'linguistic> {
    Constant(&'a PreparedGreetingRenderer<'renderer>),
    Exact(&'a PreparedExactMaterialLinguisticGreetingRenderer<'a,'renderer,'choice,'joined,'material,'linguistic>),
}
pub struct PreparedRenderedGestureOccurrence<'a,'renderer,'choice,'joined,'material,'linguistic> {
    shared: &'a PreparedIpaContextualGreetingPhoneGestures<'choice,'joined,'material>,
    renderer: ActualGestureRenderer<'a,'renderer,'choice,'joined,'material,'linguistic>,
    range: core::ops::Range<u64>,
    sample_rate_hz: u64,
}
impl<'a,'renderer,'choice,'joined,'material,'linguistic>
    PreparedRenderedGestureOccurrence<'a,'renderer,'choice,'joined,'material,'linguistic>
where 'material:'choice,'joined:'choice {
    pub fn prepare(
        intent: &'material SpeechUtteranceIntent,
        shared: &'a PreparedIpaContextualGreetingPhoneGestures<'choice,'joined,'material>,
        renderer: ActualGestureRenderer<'a,'renderer,'choice,'joined,'material,'linguistic>,
    ) -> Result<Self,RenderedOccurrenceRefusal> {
        use RenderedOccurrenceRefusal::*;
        let choice=shared.contextual().choice();
        if !core::ptr::eq(choice.occurrence().intent(),intent) { return Err(ForeignIntent); }
        let actual=match &renderer {
            ActualGestureRenderer::Constant(r)=>{
                let SpeechCycleSpecification::Known(cycle)=choice.occurrence().segment().prosody().fundamental_cycle() else { return Err(Cycle); };
                let actual=r.first_target().original_cycle();
                if u128::from(*cycle.numerator_seconds())*u128::from(*actual.denominator()) != u128::from(*actual.numerator_seconds())*u128::from(*cycle.denominator()) { return Err(Cycle); }
                *r
            },
            ActualGestureRenderer::Exact(r)=>{
                if !core::ptr::eq(r.shared(),shared) { return Err(ForeignGesture); }
                r.renderer()
            }
        };
        if !core::ptr::eq(actual.original(),shared.contextual().profile()) { return Err(ForeignGesture); }
        let first=actual.first_target();let range=first.frame_range();
        if range.start<0 || range.start>=range.end { return Err(Range); }
        let sample_rate_hz=*first.basis().sample_rate_hz();
        if sample_rate_hz==0 { return Err(Clock); }
        if let Some(second)=actual.second_target() {
            if second.frame_range()!=range || second.basis()!=first.basis() { return Err(Clock); }
        }
        Ok(Self{shared,renderer,range:range.start as u64..range.end as u64,sample_rate_hz})
    }
    pub fn shared(&self)->&PreparedIpaContextualGreetingPhoneGestures<'choice,'joined,'material> { self.shared }
    pub fn renderer(&self)->&ActualGestureRenderer<'a,'renderer,'choice,'joined,'material,'linguistic> { &self.renderer }
    pub fn frame_range(&self)->core::ops::Range<u64> { self.range.clone() }
    pub fn sample_rate_hz(&self)->u64 { self.sample_rate_hz }
}
