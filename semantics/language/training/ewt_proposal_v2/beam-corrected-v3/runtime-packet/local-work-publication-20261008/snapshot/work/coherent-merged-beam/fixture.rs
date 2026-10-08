use conduit_core::{validate_canonical_structured_value as view, PreparedStructuredComposer};
use conduit_language::*;
use conduit_plot::{PortableExpressionProgram,PreparedPortableExpressionEvaluator};
use conduit_plot::rust_binding::*;
use std::{rc::Rc,time::Instant};
const MAX:usize=262144;
fn limits()->PreparedNativeFamilyLimits{PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:MAX,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024}}
fn decode(f:&mut PreparedNativeFamily,b:&[u8],caps:&[&AdmittedNativeChild],scoped:bool,merge:bool){
 if merge {if scoped {let _=f.decode_with_admitted_children::<LanguageParserWindow8RawMerge>(b,caps,NativeChildAdmissionScope::maximum_scope_state_bytes()).unwrap();}else{let _=f.decode::<LanguageParserWindow8RawMerge>(b).unwrap();}}
 else if scoped{let _=f.decode_with_admitted_children::<LanguageParserWindow8RawBeam>(b,caps,NativeChildAdmissionScope::maximum_scope_state_bytes()).unwrap();}else{let _=f.decode::<LanguageParserWindow8RawBeam>(b).unwrap();}
}
fn main(){
 let out=std::env::args().nth(1).unwrap();
 let mut family=PreparedNativeFamily::prepare_with_child_admission(&[LanguageParserWindow8RawMerge::PREPARED_DESCRIPTOR],limits()).unwrap();
 println!("family={:?}",family.storage_receipt());
 let mut stages=Vec::new();let mut retained=0;let mut prep=0;
 for name in ["window8_rank_0_1","window8_rank_2_3","window8_rank_0_2","window8_rank_1_3","window8_rank_1_2","window8_rank_insert"]{
  let hex=std::fs::read_to_string(format!("{out}/{name}.hex")).unwrap();let hex=hex.trim();let bytes=(0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect::<Vec<_>>();
  let p=PortableExpressionProgram::from_canonical_bytes(&bytes).unwrap();let(e,r)=PreparedPortableExpressionEvaluator::new_with_storage_limits(&p,128*1024*1024,256*1024*1024,128*1024*1024).unwrap();retained+=r.retained_heap_bytes_bound+r.decoded_program_heap_bytes;prep+=r.preparation_requested_bytes_bound;stages.push((p,e));
 }
 let merge_type=LanguageParserWindow8RawMerge::semantic_type().unwrap();
 let mut composer=PreparedStructuredComposer::new(&merge_type,MAX).unwrap();
 let initial=std::fs::read("work/native-child-rank/epoch-0.bin").unwrap();let mut proposals=Vec::with_capacity(10);let mut caps=Vec::with_capacity(14);let mut cap_bound=0;
 let t=Instant::now();
 for field in ["candidate0","candidate1","candidate2","candidate3"]{let child=view(&initial).unwrap().record_field(field).unwrap().unwrap();let mut b=child.type_bytes().to_vec();b.extend_from_slice(child.value_node());let(_,c)=family.decode_admitted::<LanguageParserWindow8RawHypothesis>(Rc::from(b),usize::MAX).unwrap();cap_bound+=c.storage_receipt().combined_bytes_bound;caps.push(c);}
 for epoch in 0..10{let bytes=std::fs::read(format!("work/native-child-rank/epoch-{epoch}.bin")).unwrap();let child=view(&bytes).unwrap().record_field("candidate0").unwrap().unwrap();let mut b=child.type_bytes().to_vec();b.extend_from_slice(child.value_node());let shared:Rc<[u8]>=Rc::from(b);let(_,c)=family.decode_admitted::<LanguageParserWindow8RawHypothesis>(shared.clone(),usize::MAX).unwrap();cap_bound+=c.storage_receipt().combined_bytes_bound;caps.push(c);proposals.push(shared);}
 let cap_prep=t.elapsed().as_nanos();let refs=caps.iter().collect::<Vec<_>>();
 let mut foreign=PreparedNativeFamily::prepare_with_child_admission(&[LanguageParserWindow8RawMerge::PREPARED_DESCRIPTOR],limits()).unwrap();assert!(foreign.decode_with_admitted_children::<LanguageParserWindow8RawBeam>(&initial,&refs,NativeChildAdmissionScope::maximum_scope_state_bytes()).is_err());drop(foreign);assert!(family.decode_with_admitted_children::<LanguageParserWindow8RawBeam>(&initial,&refs,NativeChildAdmissionScope::maximum_scope_state_bytes()-1).is_err());let mut malformed=initial.clone();malformed.pop();assert!(family.decode::<LanguageParserWindow8RawBeam>(&malformed).is_err());assert!(family.decode_with_admitted_children::<LanguageParserWindow8RawBeam>(&malformed,&refs,NativeChildAdmissionScope::maximum_scope_state_bytes()).is_err());
 let mut expected=initial.clone();let mut times=[0u128;3];let mut outputs=Vec::new();
 for mode in 0..3{
  let mut beam=Vec::with_capacity(MAX);beam.extend_from_slice(&initial);let t=Instant::now();
  for ordinal in 0..38{
   for i in 0..5{let output=if mode==0{stages[i].0.evaluate(&beam).unwrap()}else{stages[i].1.evaluate(&beam).unwrap().to_vec()};decode(&mut family,&output,&refs,mode==2,false);beam.clear();beam.extend_from_slice(&output);}
   let query=composer.record(&[view(&beam).unwrap(),view(&proposals[ordinal%10]).unwrap()]).unwrap();decode(&mut family,query,&refs,mode==2,true);
   let output=if mode==0{stages[5].0.evaluate(query).unwrap()}else{stages[5].1.evaluate(query).unwrap().to_vec()};decode(&mut family,&output,&refs,mode==2,false);beam.clear();beam.extend_from_slice(&output);
   for i in 0..5{let output=if mode==0{stages[i].0.evaluate(&beam).unwrap()}else{stages[i].1.evaluate(&beam).unwrap().to_vec()};decode(&mut family,&output,&refs,mode==2,false);beam.clear();beam.extend_from_slice(&output);}
   if mode==0{outputs.push(beam.clone());}else{assert_eq!(beam,outputs[ordinal]);}
  }
  times[mode]=t.elapsed().as_nanos();expected=beam;println!("mode={mode} 38merge_418Sourcehops_nanos={}",times[mode]);
 }
 println!("PASS exact_output_parity_all38merges modesReferenceSourceFreshNative_PreparedSourceFreshNative_PreparedSourceScopedNative timings={times:?} retained_evaluators_and_programs={retained} summed_evaluator_preparation_requests_excluding_prior_program_decode={prep} caps_combined_bound={cap_bound} fresh14cap_issuance_nanos={cap_prep} fixed_scope_state={} final_encoded_bytes={}",NativeChildAdmissionScope::maximum_scope_state_bytes(),expected.len());
}
