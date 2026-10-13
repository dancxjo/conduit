//! WORK-only immutable representation reuse, not linguistic authority.
use conduit_language::{parser_window8_program_bank::Window8ProgramBank,LanguageParserRelation,LanguageParserWindow8RawClass};
use core::mem::size_of;
#[derive(Debug,PartialEq,Eq)]pub enum Refusal{Count,Context,Overflow,Retained,Peak}
pub struct Classes<'a,S>{bank:&'a Window8ProgramBank,selection:&'a S,default:&'a LanguageParserRelation,values:Vec<LanguageParserWindow8RawClass>,retained:usize}
impl<'a,S> Classes<'a,S>{
 /// Private decoder supplies all76 original Source outputs in exact code order.
 pub fn from_original(bank:&'a Window8ProgramBank,selection:&'a S,default:&'a LanguageParserRelation,values:Vec<LanguageParserWindow8RawClass>,maximum_retained:usize,maximum_peak:usize)->Result<Self,Refusal>{
  if values.len()!=76{return Err(Refusal::Count)}
  let mut heap=values.capacity().checked_mul(size_of::<LanguageParserWindow8RawClass>()).ok_or(Refusal::Overflow)?;
  for v in &values{heap=heap.checked_add(v.relation().subtype().get().capacity()).ok_or(Refusal::Overflow)?;}
  let retained=size_of::<Self>().checked_add(heap).ok_or(Refusal::Overflow)?;
  if retained>maximum_retained{return Err(Refusal::Retained)}
  // Conservative simultaneous incoming Vec header + owned representation.
  let peak=retained.checked_add(size_of::<Vec<LanguageParserWindow8RawClass>>()).ok_or(Refusal::Overflow)?;
  if peak>maximum_peak{return Err(Refusal::Peak)}
  Ok(Self{bank,selection,default,values,retained})
 }
 pub fn get(&self,bank:&Window8ProgramBank,selection:&S,default:&LanguageParserRelation,code:usize)->Result<Option<&LanguageParserWindow8RawClass>,Refusal>{
  if !core::ptr::eq(bank,self.bank)||!core::ptr::eq(selection,self.selection)||!core::ptr::eq(default,self.default){return Err(Refusal::Context)}
  Ok(self.values.get(code))
 }
 pub fn retained_bytes(&self)->usize{self.retained}
}
