//! Exact original commitment and complete renderer coverage, before queueing.
use crate::gesture_sequence::PreparedGestureSequence;
use crate::rendered_gesture_occurrence::ActualGestureRenderer;
use crate::{committed_word_pitch::PreparedCommittedWordPitch,linguistic_prosody::LinguisticProsodyBasis};
#[derive(Debug,PartialEq,Eq)]
pub enum CommittedSequenceRefusal { Count, ForeignIntent, ForeignOccurrence, ForeignPitch, ForeignCommitment }
pub struct PreparedCommittedGestureSequence<'s,'p,'a,'renderer,'choice,'joined,'material,'linguistic,'coverage,'word,'basis,'commit> {
    pitch:&'s PreparedCommittedWordPitch<'a,'coverage,'word,'basis,'commit>,
    sequence:&'s PreparedGestureSequence<'p,'a,'renderer,'choice,'joined,'material,'linguistic>,
}
impl<'s,'p,'a,'renderer,'choice,'joined,'material,'linguistic,'coverage,'word,'basis,'commit>
 PreparedCommittedGestureSequence<'s,'p,'a,'renderer,'choice,'joined,'material,'linguistic,'coverage,'word,'basis,'commit>
where 'material:'choice,'joined:'choice {
    pub fn prepare(
      pitch:&'s PreparedCommittedWordPitch<'a,'coverage,'word,'basis,'commit>,
      sequence:&'s PreparedGestureSequence<'p,'a,'renderer,'choice,'joined,'material,'linguistic>,
    )->Result<Self,CommittedSequenceRefusal> {
      use CommittedSequenceRefusal::*;
      let original=pitch.original().coverage();let actual=sequence.occurrences();
      if original.witnesses().len()!=actual.len() || original.phone_events().len()!=actual.len() {return Err(Count);}
      for ((witness,event),rendered) in original.witnesses().iter().zip(original.phone_events()).zip(actual) {
        let choice=rendered.shared().contextual().choice();
        if !core::ptr::eq(choice.occurrence().intent(),pitch.realized()) {return Err(ForeignIntent);}
        if choice.occurrence().event()!=*event || choice.occurrence().segment().occurrence()!=witness.composite().occurrence() {return Err(ForeignOccurrence);}
        let selected=pitch.segment_pitch().iter().find(|p|p.segment().occurrence()==choice.occurrence().segment().occurrence());
        match (rendered.renderer(),selected) {
          (ActualGestureRenderer::Constant(_),None)=>{},
          (ActualGestureRenderer::Exact(r),Some(segment))=>{
            if r.pitch().accepted().pitch()!=segment {return Err(ForeignPitch);}
            match r.pitch().requested() {
              LinguisticProsodyBasis::Rich(rich) if core::ptr::eq(*rich,pitch.rich().prepared())=>{},
              _=>return Err(ForeignCommitment),
            }
          },
          _=>return Err(ForeignPitch),
        }
      }
      Ok(Self{pitch,sequence})
    }
    pub fn pitch(&self)->&PreparedCommittedWordPitch<'a,'coverage,'word,'basis,'commit> {self.pitch}
    pub fn sequence(&self)->&PreparedGestureSequence<'p,'a,'renderer,'choice,'joined,'material,'linguistic> {self.sequence}
}
