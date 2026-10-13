//! Native-only representation optimization; no Source/fact/commit authority.
use crate::bank::*;
use conduit_language::*;
use conduit_plot::rust_binding::*;
use conduit_language::parser_canonical_composition::*;
use std::{rc::Rc,time::Instant};
#[derive(Default,Debug)]pub struct Stats{pub merges:usize,pub encode_nanos:u128,pub cap_issuance_nanos:u128,pub canonical_source_native_nanos:u128,pub final_native_nanos:u128,pub maximum5cap_retained_bytes:usize}
pub struct ScopedMerge{family:PreparedNativeFamily,composer:PreparedParserCanonicalComposer,stats:Stats}
impl ScopedMerge{
 pub fn prepare()->Self{
  let family=PreparedNativeFamily::prepare_with_child_admission(&[LanguageParserWindow8RawMerge::PREPARED_DESCRIPTOR],PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024}).unwrap();
  let composer=PreparedParserCanonicalComposer::prepare::<LanguageParserWindow8RawMerge>(&family,ParserCompositionLimits{maximum_output_bytes:262144,maximum_preparation_requested_bytes:64*1024*1024,maximum_retained_requested_bytes:2*1024*1024}).unwrap();
  eprintln!("scoped_merge_preparation_native={:?} composer={:?}",family.storage_receipt(),composer.receipt());Self{family,composer,stats:Stats::default()}
 }
 pub fn stats(&self)->&Stats{&self.stats}
 pub fn merge(&mut self,bank:&Window8ProgramBank,beam:LanguageParserWindow8RawBeam,proposal:LanguageParserWindow8RawHypothesis)->Result<LanguageParserWindow8RawBeam,parser_window8::Window8Refusal>{
  let t=Instant::now();let beam=beam.encode().map_err(parser_window8::Window8Refusal::Native)?;let proposal=proposal.encode().map_err(parser_window8::Window8Refusal::Native)?;self.stats.encode_nanos+=t.elapsed().as_nanos();
  let t=Instant::now();let view=conduit_core::validate_canonical_structured_value(&beam).map_err(|_|parser_window8::Window8Refusal::Program)?;let mut caps=Vec::with_capacity(5);let mut retained=0;
  for field in ["candidate0","candidate1","candidate2","candidate3"]{let child=view.record_field(field).map_err(|_|parser_window8::Window8Refusal::Program)?.ok_or(parser_window8::Window8Refusal::Program)?;let mut bytes=child.type_bytes().to_vec();bytes.extend_from_slice(child.value_node());let(_,cap)=self.family.decode_admitted::<LanguageParserWindow8RawHypothesis>(Rc::from(bytes),1024*1024).map_err(parser_window8::Window8Refusal::Native)?;retained+=cap.storage_receipt().combined_bytes_bound;caps.push(cap);}
  let(_,cap)=self.family.decode_admitted::<LanguageParserWindow8RawHypothesis>(Rc::from(proposal.clone()),1024*1024).map_err(parser_window8::Window8Refusal::Native)?;retained+=cap.storage_receipt().combined_bytes_bound;caps.push(cap);self.stats.maximum5cap_retained_bytes=self.stats.maximum5cap_retained_bytes.max(retained);self.stats.cap_issuance_nanos+=t.elapsed().as_nanos();let refs=caps.iter().collect::<Vec<_>>();
  let t=Instant::now();let frame=bank.scoped_rank_merge_fixture(&beam,&proposal,&mut self.family,&refs,&mut self.composer)?;self.stats.canonical_source_native_nanos+=t.elapsed().as_nanos();
  let t=Instant::now();let value=self.family.decode_with_admitted_children::<LanguageParserWindow8RawBeam>(&frame,&refs,NativeChildAdmissionScope::maximum_scope_state_bytes()).map_err(parser_window8::Window8Refusal::Native)?;self.stats.final_native_nanos+=t.elapsed().as_nanos();self.stats.merges+=1;Ok(value)
 }
}
