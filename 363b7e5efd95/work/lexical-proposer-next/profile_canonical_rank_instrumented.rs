use conduit_language::{parser_window8_program_bank::*,*};
use conduit_plot::rust_binding::{NativeRustBinding,PreparedNativeFamilyLimits};
use std::time::Instant;
#[path="finite_state_proof_owner.rs"]mod reuse;
fn timed<T>(name:&str,f:impl FnOnce()->T)->T{let t=Instant::now();let v=f();println!("phase={name} elapsed_nanos={}",t.elapsed().as_nanos());v}
#[path="canonical_rank_owner_instrumented.rs"]mod canonical;
fn main(){
 let p=std::env::args().nth(1).unwrap();let j:serde_json::Value=serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
 let hex=j["receipts"][0]["epochs"][0]["beam_canonical"].as_str().unwrap();let bytes=(0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect::<Vec<_>>();
 let beam=timed("original_native_beam_decode",||LanguageParserWindow8RawBeam::decode(&bytes).unwrap());
 let bank=timed("complete_bank_preparation",||Window8ProgramBank::prepare_proposal_v2_native_evaluator(PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024},Window8SourcePreparationLimits{maximum_retained_bytes:512*1024*1024,maximum_preparation_peak_bytes:1024*1024*1024,maximum_input_bytes:262144}).unwrap());
 let out=std::path::Path::new("target/debug/build/conduit-language-70b796a2da20ac23/out");
 let mut canonical=timed("canonical_rank_owner_preparation",||canonical::RankOwner::prepare(out));
 let reference=timed("original_rank_five_hops",||bank.rank(beam.clone()).unwrap());
 let optimized=timed("canonical_rank_five_full_native_hops",||canonical.rank(beam.clone()));assert_eq!(optimized,reference);
 println!("PASS original_five_rank_Source_full_Native_intermediate_parity");
 let state=beam.candidate0().state();let basis=state.basis();
 let proof=timed("original_complete_forest_admission",||bank.admit_state(state).unwrap());
 let expected=timed("original_completion",||bank.complete(&proof).unwrap());
 let _context=timed("original_move_context",||bank.context(&proof,basis).unwrap());
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
 assert!(bank.class(76,&default).is_err());println!("PASS exact_original_outputs_foreign_context_and_class_refusal");
}
