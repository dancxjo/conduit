//! WORK-only immutable representation reuse, not linguistic authority.
use conduit_language::{parser_window8_program_bank::Window8ProgramBank,LanguageParserRelation,LanguageParserWindow8RawClass};
use core::mem::size_of;
#[derive(Debug,PartialEq,Eq)]pub enum Refusal{Count,Context,Overflow,Retained,Peak}
/// Exact original Source outputs in immutable code order; no caller Vec entrance.
pub struct OriginalClasses<'a,S>{bank:&'a Window8ProgramBank,selection:&'a S,default:&'a LanguageParserRelation,values:Vec<LanguageParserWindow8RawClass>}
impl<'a,S> OriginalClasses<'a,S>{
 pub fn derive(bank:&'a Window8ProgramBank,selection:&'a S,default:&'a LanguageParserRelation)->Result<Self,conduit_language::parser_window8::Window8Refusal>{
  let mut values=Vec::with_capacity(76);for code in 0..76{values.push(bank.class(code,default)?)}
  Ok(Self{bank,selection,default,values})
 }
 pub fn values(&self)->&[LanguageParserWindow8RawClass]{&self.values}
}
impl<S> Clone for OriginalClasses<'_,S>{fn clone(&self)->Self{Self{bank:self.bank,selection:self.selection,default:self.default,values:self.values.clone()}}}
pub struct Classes<'a,S>{bank:&'a Window8ProgramBank,selection:&'a S,default:&'a LanguageParserRelation,values:Vec<LanguageParserWindow8RawClass>,retained:usize}
impl<'a,S> Classes<'a,S>{
 /// Consumes opaque Source-derived all76 outputs; Source derivation allocation peak is outside this move-only component receipt.
 pub fn from_original(bank:&'a Window8ProgramBank,selection:&'a S,default:&'a LanguageParserRelation,original:OriginalClasses<'a,S>,maximum_retained:usize,maximum_peak:usize)->Result<Self,Refusal>{
  if !core::ptr::eq(bank,original.bank)||!core::ptr::eq(selection,original.selection)||!core::ptr::eq(default,original.default){return Err(Refusal::Context)}
  let values=original.values;
  if values.len()!=76{return Err(Refusal::Count)}
  let mut heap=values.capacity().checked_mul(size_of::<LanguageParserWindow8RawClass>()).ok_or(Refusal::Overflow)?;
  for v in &values{heap=heap.checked_add(v.relation().subtype().get().capacity()).ok_or(Refusal::Overflow)?;}
  let retained=size_of::<Self>().checked_add(heap).ok_or(Refusal::Overflow)?;
  if retained>maximum_retained{return Err(Refusal::Retained)}
  // Conservative simultaneous incoming Vec header + owned representation.
  let peak=retained.checked_add(size_of::<OriginalClasses<S>>()).ok_or(Refusal::Overflow)?;
  if peak>maximum_peak{return Err(Refusal::Peak)}
  Ok(Self{bank,selection,default,values,retained})
 }
 pub fn get(&self,bank:&Window8ProgramBank,selection:&S,default:&LanguageParserRelation,code:usize)->Result<Option<&LanguageParserWindow8RawClass>,Refusal>{
  if !core::ptr::eq(bank,self.bank)||!core::ptr::eq(selection,self.selection)||!core::ptr::eq(default,self.default){return Err(Refusal::Context)}
  Ok(self.values.get(code))
 }
 pub fn retained_bytes(&self)->usize{self.retained}
}
