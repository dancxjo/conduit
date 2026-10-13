use conduit_language::{parser_window8_program_bank::*,*};
use conduit_plot::rust_binding::{NativeRustBinding,PreparedNativeFamilyLimits};
use std::time::Instant;
#[path="/home/dancxjo/conduit-4907-evaluation-identity/work/lexical-proposer-next/finite_state_proof_owner.rs"]mod reuse;
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
 let revision=123u64;let selection=456u64;let foreign=789u64;
 let mut owner=reuse::Owner::new(&bank,&revision,&selection,basis,1024*1024,2*1024*1024).unwrap();
 timed("moved_proof_remember",||owner.remember(&bank,&revision,&selection,proof).unwrap());
 let cached=timed("exact_proof_lookup",||owner.lookup(&bank,&revision,&selection,state).unwrap().unwrap());
 assert_eq!(cached.state(),state);assert_eq!(bank.complete(cached).unwrap(),expected);
 assert_eq!(owner.lookup(&bank,&foreign,&selection,state).err(),Some(reuse::Refusal::ForeignContext));
 let default=LanguageParserRelation::new(LanguageUniversalDependencyRelation::Dep,LanguageParserSubtype::new(String::new()).unwrap()).unwrap();
 let classes=timed("original_complete_76_class_derivation",||(0..76).map(|c|bank.class(c,&default).unwrap()).collect::<Vec<_>>());
 let mut heap=classes.capacity()*std::mem::size_of::<LanguageParserWindow8RawClass>();
 for c in &classes {heap+=c.relation().subtype().get().capacity();}
 println!("class_retained_bytes={heap}");
 timed("all_76_original_class_output_parity",||for (i,c) in classes.iter().enumerate(){assert_eq!(&bank.class(i as u64,&default).unwrap(),c)});
 let mut proposed=0u128;let mut advances=0u128;let mut merges=0u128;let mut constructors=0u128;let mut accepted=0;
 let mut merged=beam.clone();
 for (i,class) in classes.iter().enumerate(){
  let t=Instant::now();let proposal=context.propose(class).unwrap();proposed+=t.elapsed().as_nanos();
  if !proposal.proposal().accepted(){continue} accepted+=1;
  let t=Instant::now();let input=LanguageParserWindow8RawAdvance::new(*beam.candidate0().choices(),1000+i as u64,proposal.proposal().clone(),*beam.candidate0().score(),*beam.candidate0().selected(),0).unwrap();constructors+=t.elapsed().as_nanos();
  let t=Instant::now();let raw=bank.score_advance(input).unwrap();advances+=t.elapsed().as_nanos();
  let t=Instant::now();merged=bank.merge(merged,raw).unwrap();merges+=t.elapsed().as_nanos();
 }
 println!("all76_context_proposals_nanos={proposed} accepted={accepted} raw_advance_constructors_nanos={constructors} original_score_advance_nanos={advances} original_merge_nanos={merges}");
 assert!(bank.class(76,&default).is_err());println!("PASS exact_original_outputs_foreign_context_and_class_refusal");
}
