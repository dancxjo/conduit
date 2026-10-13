use conduit_language::{parser_window8_program_bank::*,*};
use conduit_plot::rust_binding::{NativeRustBinding,PreparedNativeFamilyLimits};
use std::time::Instant;
#[path="finite_state_proof_owner_v2.rs"]mod reuse;
fn timed<T>(name:&str,f:impl FnOnce()->T)->T{let t=Instant::now();let v=f();println!("phase={name} elapsed_nanos={}",t.elapsed().as_nanos());v}
#[path="move_allocation_probe.rs"]mod allocation;
fn main(){
 let p=std::env::args().nth(1).unwrap();let j:serde_json::Value=serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
 let hex=j["receipts"][0]["epochs"][0]["beam_canonical"].as_str().unwrap();let bytes=(0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect::<Vec<_>>();
 let beam=timed("original_native_beam_decode",||LanguageParserWindow8RawBeam::decode(&bytes).unwrap());
 let bank=timed("complete_bank_preparation",||Window8ProgramBank::prepare_proposal_v2_native_evaluator(PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024},Window8SourcePreparationLimits{maximum_retained_bytes:512*1024*1024,maximum_preparation_peak_bytes:1024*1024*1024,maximum_input_bytes:262144}).unwrap());
 let state=beam.candidate0().state();let basis=state.basis();
 let proof=timed("original_complete_forest_admission",||bank.admit_state(state).unwrap());
 let expected=timed("original_completion",||bank.complete(&proof).unwrap());
 let _context=timed("original_move_context",||bank.context(&proof,basis).unwrap());
 let revision=123u64;let selection=456u64;let foreign=789u64;
 let mut owner=reuse::Owner::new(&bank,&revision,&selection,basis,1024*1024,2*1024*1024).unwrap();
 let duplicate=proof.clone();
 let (_,allocations)=allocation::allocations(||owner.remember(&bank,&revision,&selection,proof).unwrap());assert_eq!(allocations,0);
 let retained=owner.retained_bytes().unwrap();
 let mut under=reuse::Owner::new(&bank,&revision,&selection,basis,retained-1,2*1024*1024).unwrap();
 assert_eq!(under.remember(&bank,&revision,&selection,duplicate.clone()).err(),Some(reuse::Refusal::Retained));
 let mut peak_under=reuse::Owner::new(&bank,&revision,&selection,basis,1024*1024,core::mem::size_of_val(&owner)).unwrap();
 assert_eq!(peak_under.remember(&bank,&revision,&selection,duplicate.clone()).err(),Some(reuse::Refusal::Peak));
 let (_,allocations)=allocation::allocations(||{assert!(owner.lookup(&bank,&revision,&selection,state).unwrap().is_some());});assert_eq!(allocations,0);
 for changed in 0..8 {
  let mut heads=*state.heads();let mut stack=*state.stack();if changed==0{heads[0]=if heads[0]==9{8}else{9};}if changed==1{stack[0]=if stack[0]==8{0}else{8};}
  let other=LanguageParserWindow8RawState::new(basis.clone(),*state.committed()+u64::from(changed==2),*state.depth()+u64::from(changed==3),heads,state.relation0().clone(),state.relation1().clone(),state.relation2().clone(),state.relation3().clone(),state.relation4().clone(),state.relation5().clone(),state.relation6().clone(),state.relation7().clone(),stack,*state.token_count()+u64::from(changed==4),*state.unread()+u64::from(changed==5)).unwrap();
  if changed<6 {assert!(owner.lookup(&bank,&revision,&selection,&other).unwrap().is_none());}
 }
 println!("PASS zero_allocation_lookup_move_changed_six_fields_one_under_retained_peak retained_bytes={retained}");
 let cached=timed("exact_proof_lookup",||owner.lookup(&bank,&revision,&selection,state).unwrap().unwrap());
 assert_eq!(cached.state(),state);assert_eq!(bank.complete(cached).unwrap(),expected);
 assert_eq!(owner.lookup(&bank,&foreign,&selection,state).err(),Some(reuse::Refusal::ForeignContext));
 let default=LanguageParserRelation::new(LanguageUniversalDependencyRelation::Dep,LanguageParserSubtype::new(String::new()).unwrap()).unwrap();
 let classes=timed("original_complete_76_class_derivation",||(0..76).map(|c|bank.class(c,&default).unwrap()).collect::<Vec<_>>());
 let mut heap=classes.capacity()*std::mem::size_of::<LanguageParserWindow8RawClass>();
 for c in &classes {heap+=c.relation().subtype().get().capacity();}
 println!("class_retained_bytes={heap}");
 timed("all_76_original_class_output_parity",||for (i,c) in classes.iter().enumerate(){assert_eq!(&bank.class(i as u64,&default).unwrap(),c)});
 assert!(bank.class(76,&default).is_err());println!("PASS exact_original_outputs_foreign_context_and_class_refusal");
}
