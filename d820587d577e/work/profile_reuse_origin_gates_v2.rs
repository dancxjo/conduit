use conduit_language::{parser_window8_program_bank::*,*};
use conduit_plot::rust_binding::{NativeRustBinding,PreparedNativeFamilyLimits};
use std::time::Instant;
#[path="finite_state_proof_owner_v3.rs"]mod reuse;
fn timed<T>(name:&str,f:impl FnOnce()->T)->T{let t=Instant::now();let v=f();println!("phase={name} elapsed_nanos={}",t.elapsed().as_nanos());v}
#[path="move_allocation_probe.rs"]mod allocation;
#[path="finite_class_owner_v2.rs"]mod class_owner;
fn main(){
 let p=std::env::args().nth(1).unwrap();let j:serde_json::Value=serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
 let hex=j["receipts"][0]["epochs"][0]["beam_canonical"].as_str().unwrap();let bytes=(0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect::<Vec<_>>();
 let beam=timed("original_native_beam_decode",||LanguageParserWindow8RawBeam::decode(&bytes).unwrap());
 let bank=timed("complete_bank_preparation",||Window8ProgramBank::prepare_proposal_v2_native_evaluator(PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024},Window8SourcePreparationLimits{maximum_retained_bytes:512*1024*1024,maximum_preparation_peak_bytes:1024*1024*1024,maximum_input_bytes:262144}).unwrap());
 let state=beam.candidate0().state();let basis=state.basis();
 let revision=123u64;let selection=456u64;let foreign=789u64;
 let proof=timed("original_complete_forest_admission",||reuse::OriginalProof::admit(&bank,&revision,&selection,state).unwrap());
 let expected=timed("original_completion",||bank.complete(proof.admitted()).unwrap());
 let _context=timed("original_move_context",||bank.context(proof.admitted(),basis).unwrap());
 let mut owner=reuse::Owner::new(&bank,&revision,&selection,basis,1024*1024,2*1024*1024).unwrap();
 let duplicate=proof.clone();
 let (_,allocations)=allocation::allocations(||owner.remember(&bank,&revision,&selection,proof).unwrap());assert_eq!(allocations,0);
 let retained=owner.retained_bytes().unwrap();
 let mut under=reuse::Owner::new(&bank,&revision,&selection,basis,retained-1,2*1024*1024).unwrap();
 assert_eq!(under.remember(&bank,&revision,&selection,duplicate.clone()).err(),Some(reuse::Refusal::Retained));
 let mut peak_under=reuse::Owner::new(&bank,&revision,&selection,basis,1024*1024,core::mem::size_of_val(&owner)).unwrap();
 assert_eq!(peak_under.remember(&bank,&revision,&selection,duplicate.clone()).err(),Some(reuse::Refusal::Peak));
 let (_,allocations)=allocation::allocations(||{assert!(owner.lookup(&bank,&revision,&selection,state).unwrap().is_some());});assert_eq!(allocations,0);
 for changed in 0..9 {
  let mut heads=*state.heads();let mut stack=*state.stack();if changed==0{heads[0]=if heads[0]==9{8}else{9};}if changed==1{stack[0]=if stack[0]==8{0}else{8};}
  let other_basis=if changed==8{LanguageParserBasis::new(LanguageAnalysisRevisionId::new("foreign-analysis".into()).unwrap(),basis.source_revision().clone(),basis.text().clone()).unwrap()}else{basis.clone()};
  let relation0=if changed==6{LanguageParserRelation::new(state.relation0().base().clone(),LanguageParserSubtype::new("different".into()).unwrap()).unwrap()}else{state.relation0().clone()};
  let other=LanguageParserWindow8RawState::new(other_basis,*state.committed()+u64::from(changed==2),*state.depth()+u64::from(changed==3),heads,relation0,state.relation1().clone(),state.relation2().clone(),state.relation3().clone(),state.relation4().clone(),state.relation5().clone(),state.relation6().clone(),state.relation7().clone(),stack,*state.token_count()+u64::from(changed==4),*state.unread()+u64::from(changed==5)).unwrap();
  if changed<7 {assert!(owner.lookup(&bank,&revision,&selection,&other).unwrap().is_none());}if changed==8{assert_eq!(owner.lookup(&bank,&revision,&selection,&other).err(),Some(reuse::Refusal::Basis));}
 }
 let reference_bank=Window8ProgramBank::prepare().unwrap();
 assert_eq!(owner.lookup(&reference_bank,&revision,&selection,state).err(),Some(reuse::Refusal::ForeignContext));
 let foreign_proof=reuse::OriginalProof::admit(&reference_bank,&revision,&selection,state).unwrap();assert_eq!(owner.remember(&bank,&revision,&selection,foreign_proof).err(),Some(reuse::Refusal::ForeignContext));
 let mut cycle=*state.heads();cycle[0]=0;
 let invalid=LanguageParserWindow8RawState::new(basis.clone(),*state.committed(),*state.depth(),cycle,state.relation0().clone(),state.relation1().clone(),state.relation2().clone(),state.relation3().clone(),state.relation4().clone(),state.relation5().clone(),state.relation6().clone(),state.relation7().clone(),*state.stack(),*state.token_count(),*state.unread()).unwrap();
 let prepared_error=format!("{:?}",bank.admit_state(&invalid).err().expect("cycle must refuse"));
 let reference_error=format!("{:?}",reference_bank.admit_state(&invalid).err().expect("cycle must refuse"));assert_eq!(prepared_error,reference_error);
 println!("PASS relation_basis_foreign_bank_cycle_reference_refusal_parity {prepared_error}");
 println!("PASS zero_allocation_lookup_move_changed_six_fields_one_under_retained_peak retained_bytes={retained}");
 let cached=timed("exact_proof_lookup",||owner.lookup(&bank,&revision,&selection,state).unwrap().unwrap());
 assert_eq!(cached.state(),state);assert_eq!(bank.complete(cached).unwrap(),expected);
 assert_eq!(owner.lookup(&bank,&foreign,&selection,state).err(),Some(reuse::Refusal::ForeignContext));
 let default=LanguageParserRelation::new(LanguageUniversalDependencyRelation::Dep,LanguageParserSubtype::new(String::new()).unwrap()).unwrap();
 let classes=timed("original_complete_76_class_derivation",||class_owner::OriginalClasses::derive(&bank,&selection,&default).unwrap());
 let mut heap=classes.values().len()*std::mem::size_of::<LanguageParserWindow8RawClass>();
 for c in classes.values() {heap+=c.relation().subtype().get().capacity();}
 println!("class_retained_bytes={heap}");
 timed("all_76_original_class_output_parity",||for (i,c) in classes.values().iter().enumerate(){assert_eq!(&reference_bank.class(i as u64,&default).unwrap(),c)});
 assert_eq!(owner.lookup(&bank,&revision,&selection,state).unwrap().unwrap().proof(),reference_bank.admit_state(state).unwrap().proof());
 let duplicate=classes.clone();
 let (classes,allocations)=allocation::allocations(||class_owner::Classes::from_original(&bank,&selection,&default,classes,1024*1024,2*1024*1024).unwrap());assert_eq!(allocations,0);
 let retained=classes.retained_bytes();assert_eq!(class_owner::Classes::from_original(&bank,&selection,&default,duplicate.clone(),retained-1,2*1024*1024).err(),Some(class_owner::Refusal::Retained));
 assert_eq!(class_owner::Classes::from_original(&bank,&selection,&default,duplicate.clone(),1024*1024,retained+core::mem::size_of::<class_owner::OriginalClasses<u64>>()-1).err(),Some(class_owner::Refusal::Peak));
 let (_,allocations)=allocation::allocations(||{for (i,v) in duplicate.values().iter().enumerate(){assert_eq!(classes.get(&bank,&selection,&default,i).unwrap().unwrap(),v)}assert!(classes.get(&bank,&selection,&default,76).unwrap().is_none());});assert_eq!(allocations,0);
 assert_eq!(classes.get(&reference_bank,&selection,&default,0).err(),Some(class_owner::Refusal::Context));assert_eq!(classes.get(&bank,&foreign,&default,0).err(),Some(class_owner::Refusal::Context));
 println!("PASS class_move_lookup_zero_alloc_one_under_retained_peak_foreign_context retained_bytes={retained}");
 let foreign_default=default.clone();assert_eq!(classes.get(&bank,&selection,&foreign_default,0).err(),Some(class_owner::Refusal::Context));
 assert_eq!(format!("{:?}",bank.class(76,&default).err()),format!("{:?}",reference_bank.class(76,&default).err()));
 assert!(bank.class(76,&default).is_err());println!("PASS exact_original_outputs_foreign_context_and_class_refusal");
}
