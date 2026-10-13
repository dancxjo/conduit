//! Private staging only. No production selection or new admission authority.
use conduit_language::{parser_window8_program_bank::{Window8BankState,Window8ProgramBank},LanguageParserBasis,LanguageParserWindow8RawState};
use core::mem::size_of;
#[derive(Debug,PartialEq,Eq)]
pub enum Refusal { ForeignContext, Basis, Overflow, Retained, Peak }
/// Opaque provenance for a proof produced by this exact original bank call.
/// Original admission costs remain outside the subsequent move-only cache receipt.
pub struct OriginalProof<'a,R,S>{bank:&'a Window8ProgramBank,revision:&'a R,selection:&'a S,proof:Window8BankState}
impl<'a,R,S> OriginalProof<'a,R,S>{
 pub fn admit(bank:&'a Window8ProgramBank,revision:&'a R,selection:&'a S,state:&LanguageParserWindow8RawState)->Result<Self,conduit_language::parser_window8::Window8Refusal>{
  Ok(Self{bank,revision,selection,proof:bank.admit_state(state)?})
 }
 pub fn admitted(&self)->&Window8BankState{&self.proof}
}
impl<R,S> Clone for OriginalProof<'_,R,S>{fn clone(&self)->Self{Self{bank:self.bank,revision:self.revision,selection:self.selection,proof:self.proof.clone()}}}
pub struct Owner<'a,R,S> {
    bank:&'a Window8ProgramBank, revision:&'a R, selection:&'a S,
    basis:&'a LanguageParserBasis, slots:[Option<Window8BankState>;4],
    next:usize, maximum_retained:usize, maximum_peak:usize,
}
impl<'a,R,S> Owner<'a,R,S> {
    pub fn new(bank:&'a Window8ProgramBank,revision:&'a R,selection:&'a S,basis:&'a LanguageParserBasis,maximum_retained:usize,maximum_peak:usize)->Result<Self,Refusal>{
        if size_of::<Self>()>maximum_retained{return Err(Refusal::Retained)}
        if size_of::<Self>()>maximum_peak{return Err(Refusal::Peak)}
        Ok(Self{bank,revision,selection,basis,slots:core::array::from_fn(|_|None),next:0,maximum_retained,maximum_peak})
    }
    pub fn retained_bytes(&self)->Result<usize,Refusal>{
        self.slots.iter().flatten().try_fold(size_of::<Self>(),|v,p|v.checked_add(heap(p.state())?).ok_or(Refusal::Overflow))
    }
    fn context(&self,bank:&Window8ProgramBank,revision:&R,selection:&S)->Result<(),Refusal>{
        if !core::ptr::eq(self.bank,bank)||!core::ptr::eq(self.revision,revision)||!core::ptr::eq(self.selection,selection){return Err(Refusal::ForeignContext)}Ok(())
    }
    pub fn lookup(&self,bank:&Window8ProgramBank,revision:&R,selection:&S,state:&LanguageParserWindow8RawState)->Result<Option<&Window8BankState>,Refusal>{
        self.context(bank,revision,selection)?;
        if state.basis()!=self.basis{return Err(Refusal::Basis)}
        Ok(self.slots.iter().flatten().find(|p|p.state()==state))
    }
    /// Caller must supply the moved result of this exact bank's original admission.
    /// This private representation owner neither creates nor weakens that proof.
    pub fn remember(&mut self,bank:&Window8ProgramBank,revision:&R,selection:&S,original:OriginalProof<'a,R,S>)->Result<(),Refusal>{
        self.context(bank,revision,selection)?;
        self.context(original.bank,original.revision,original.selection)?;
        let proof=original.proof;
        if proof.state().basis()!=self.basis{return Err(Refusal::Basis)}
        let incoming=heap(proof.state())?;
        let mut peak=size_of::<Self>().checked_add(size_of::<OriginalProof<'a,R,S>>()).and_then(|v|v.checked_add(incoming)).ok_or(Refusal::Overflow)?;
        for p in self.slots.iter().flatten(){peak=peak.checked_add(heap(p.state())?).ok_or(Refusal::Overflow)?;}
        if peak>self.maximum_peak{return Err(Refusal::Peak)}
        let mut total=size_of::<Self>();
        for (i,p) in self.slots.iter().enumerate(){if i!=self.next {if let Some(p)=p {total=total.checked_add(heap(p.state())?).ok_or(Refusal::Overflow)?;}}}
        total=total.checked_add(heap(proof.state())?).ok_or(Refusal::Overflow)?;
        if total>self.maximum_retained{return Err(Refusal::Retained)}
        self.slots[self.next]=Some(proof);self.next=(self.next+1)%4;Ok(())
    }
}
fn heap(s:&LanguageParserWindow8RawState)->Result<usize,Refusal>{
    let b=s.basis();
    let mut total=0usize;
    for capacity in [b.analysis_revision().get().capacity(),b.source_revision().get().capacity(),b.text().get().capacity()]{total=total.checked_add(capacity).ok_or(Refusal::Overflow)?;}
    for r in [s.relation0(),s.relation1(),s.relation2(),s.relation3(),s.relation4(),s.relation5(),s.relation6(),s.relation7()]{total=total.checked_add(r.subtype().get().capacity()).ok_or(Refusal::Overflow)?;}
    Ok(total)
}
