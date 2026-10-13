extern crate alloc;use conduit_language::*;
#[path="parser_window8_program_bank.rs"]mod bank;use bank::*;
use conduit_plot::rust_binding::{NativeRustBinding,PreparedNativeFamilyLimits};
use std::time::Instant;

fn timed<T>(name:&str,f:impl FnOnce()->T)->T{let t=Instant::now();let v=f();println!("phase={name} elapsed_nanos={}",t.elapsed().as_nanos());v}
fn main(){
 let p=std::env::args().nth(1).unwrap();let j:serde_json::Value=serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
 let hex=j["receipts"][0]["epochs"][0]["beam_canonical"].as_str().unwrap();let bytes=(0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect::<Vec<_>>();
 let beam=timed("original_native_beam_decode",||LanguageParserWindow8RawBeam::decode(&bytes).unwrap());
 let bank=timed("complete_bank_preparation",||Window8ProgramBank::prepare_proposal_v2_native_evaluator(PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024},Window8SourcePreparationLimits{maximum_retained_bytes:512*1024*1024,maximum_preparation_peak_bytes:1024*1024*1024,maximum_input_bytes:262144}).unwrap());
 let state=beam.candidate0().state();let basis=state.basis();
 let proof=timed("original_complete_forest_admission",||bank.admit_state(state).unwrap());
 let expected=timed("original_completion",||bank.complete(&proof).unwrap());
 let context=timed("original_move_context",||bank.context(&proof,basis).unwrap());
 let default=LanguageParserRelation::new(LanguageUniversalDependencyRelation::Dep,LanguageParserSubtype::new(String::new()).unwrap()).unwrap();
 let classes=timed("original_complete_76_class_derivation",||(0..76).map(|c|bank.class(c,&default).unwrap()).collect::<Vec<_>>());
 let mut heap=classes.capacity()*std::mem::size_of::<LanguageParserWindow8RawClass>();
 for c in &classes {heap+=c.relation().subtype().get().capacity();}
 println!("class_retained_bytes={heap}");
 timed("all_76_original_class_output_parity",||for (i,c) in classes.iter().enumerate(){assert_eq!(&bank.class(i as u64,&default).unwrap(),c)});
 let mut proposed=0u128;let mut advances=0u128;let mut merges=0u128;let mut constructors=0u128;let mut accepted=0;
 let mut merged=beam.clone();let mut raw_proposals=Vec::with_capacity(76);let mut original_outputs=Vec::with_capacity(76);
 for (i,class) in classes.iter().enumerate(){
  let t=Instant::now();let proposal=context.propose(class).unwrap();proposed+=t.elapsed().as_nanos();
  if !proposal.proposal().accepted(){continue} accepted+=1;
  let t=Instant::now();let input=LanguageParserWindow8RawAdvance::new(*beam.candidate0().choices(),1000+i as u64,proposal.proposal().clone(),*beam.candidate0().score(),*beam.candidate0().selected(),0).unwrap();constructors+=t.elapsed().as_nanos();
  let t=Instant::now();let raw=bank.score_advance(input).unwrap();advances+=t.elapsed().as_nanos();
  let t=Instant::now();merged=bank.merge(merged,raw.clone()).unwrap();merges+=t.elapsed().as_nanos();raw_proposals.push(raw);original_outputs.push(merged.clone().encode().unwrap());
 }
 println!("all76_context_proposals_nanos={proposed} accepted={accepted} raw_advance_constructors_nanos={constructors} original_score_advance_nanos={advances} original_merge_nanos={merges}");
 use conduit_plot::rust_binding::{PreparedNativeFamily,PreparedNativeRustBinding,NativeChildAdmissionScope};use std::rc::Rc;
 let mut family=PreparedNativeFamily::prepare_with_child_admission(&[LanguageParserWindow8RawMerge::PREPARED_DESCRIPTOR],PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024}).unwrap();
 let mut composer=conduit_language::parser_canonical_composition::PreparedParserCanonicalComposer::prepare::<LanguageParserWindow8RawMerge>(&family,conduit_language::parser_canonical_composition::ParserCompositionLimits{maximum_output_bytes:262144,maximum_preparation_requested_bytes:64*1024*1024,maximum_retained_requested_bytes:2*1024*1024}).unwrap();
 println!("additional_scoped_family={:?} bounded_merge_composer={:?}",family.storage_receipt(),composer.receipt());
 let mut frame=beam.clone().encode().unwrap();let mut scoped=0u128;let mut encoding=0u128;let mut issuance=0u128;let mut maximum_cap=0usize;
 for (index,raw) in raw_proposals.into_iter().enumerate(){
  let t=Instant::now();let proposal=raw.encode().unwrap();encoding+=t.elapsed().as_nanos();
  let t=Instant::now();let v=conduit_core::validate_canonical_structured_value(&frame).unwrap();let mut caps=Vec::with_capacity(5);let mut capbytes=0;
  for field in ["candidate0","candidate1","candidate2","candidate3"]{let child=v.record_field(field).unwrap().unwrap();let mut b=child.type_bytes().to_vec();b.extend_from_slice(child.value_node());let(_,cap)=family.decode_admitted::<LanguageParserWindow8RawHypothesis>(Rc::from(b),1024*1024).unwrap();capbytes+=cap.storage_receipt().combined_bytes_bound;caps.push(cap);}
  let(_,cap)=family.decode_admitted::<LanguageParserWindow8RawHypothesis>(Rc::from(proposal.clone()),1024*1024).unwrap();capbytes+=cap.storage_receipt().combined_bytes_bound;caps.push(cap);maximum_cap=maximum_cap.max(capbytes);issuance+=t.elapsed().as_nanos();let refs=caps.iter().collect::<Vec<_>>();
  let t=Instant::now();frame=bank.scoped_rank_merge_fixture(&frame,&proposal,&mut family,&refs,&mut composer).unwrap();scoped+=t.elapsed().as_nanos();assert_eq!(frame,original_outputs[index]);
 }
 println!("PASS all_actual_accepted_76class_outputs_original_merges_parity scoped_merges_nanos={scoped} incoming_proposal_encode_nanos={encoding} fresh_permerge5cap_issuance_nanos={issuance} maximum5cap_combined_bound={maximum_cap} scope_logical_bytes={}",NativeChildAdmissionScope::maximum_scope_state_bytes());
 assert!(bank.class(76,&default).is_err());println!("PASS exact_original_outputs_and_class_refusal");
}
