use conduit_plot::{PortableExpressionProgram,PreparedPortableExpressionEvaluator};
use conduit_plot::rust_binding::{PreparedNativeFamily,PreparedNativeFamilyLimits,PreparedNativeRustBinding,NativeChildAdmissionScope};
use std::{rc::Rc,time::Instant};
fn main(){
 let limits=PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024};
 let mut family=PreparedNativeFamily::prepare_with_child_admission(PREPARED_NATIVE_FAMILY_ROOTS,limits).unwrap();
 let dir="/home/dancxjo/conduit-4907-evaluation-identity/target/debug/build/conduit-language-70b796a2da20ac23/out";
 let mut stages=Vec::new();let mut retained=0usize;let mut prep=0usize;
 for name in ["window8_rank_0_1","window8_rank_2_3","window8_rank_0_2","window8_rank_1_3","window8_rank_1_2"]{
  let hex=std::fs::read_to_string(format!("{dir}/{name}.hex")).unwrap();let hex=hex.trim();let bytes=(0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect::<Vec<_>>();let program=PortableExpressionProgram::from_canonical_bytes(&bytes).unwrap();
  let (evaluator,receipt)=PreparedPortableExpressionEvaluator::new_with_storage_limits(&program,128*1024*1024,256*1024*1024,128*1024*1024).unwrap();retained+=receipt.retained_heap_bytes_bound+receipt.decoded_program_heap_bytes;prep+=receipt.preparation_requested_bytes_bound;stages.push((program,evaluator));
 }
 println!("five_stage_evaluator_retained={retained} summed_preparation_requests={prep} buffer_reservation={}",2*262144);
 let mut ordinary=0u128;let mut canonical=0u128;let mut scoped=0u128;let mut full_source=0u128;
 for epoch in 0..10{
  let bytes=std::fs::read(format!("work/native-child-rank/epoch-{epoch}.bin")).unwrap();let view=conduit_core::validate_canonical_structured_value(&bytes).unwrap();let mut caps=Vec::new();
  for field in ["candidate0","candidate1","candidate2","candidate3"]{
   let child=view.record_field(field).unwrap().unwrap();let mut frame=child.type_bytes().to_vec();frame.extend_from_slice(child.value_node());let (_,cap)=family.decode_admitted::<LanguageParserWindow8RawHypothesis>(Rc::from(frame),usize::MAX).unwrap();caps.push(cap);
  }let refs=caps.iter().collect::<Vec<_>>();
  let start=Instant::now();let mut expected=bytes.clone();for(program,_)in &stages{expected=program.evaluate(&expected).unwrap();let _=LanguageParserWindow8RawBeam::decode(&expected).unwrap();}full_source+=start.elapsed().as_nanos();
  let start=Instant::now();let mut value=LanguageParserWindow8RawBeam::decode(&bytes).unwrap();for(_,evaluator)in &mut stages{let input=value.encode().unwrap();value=family.decode::<LanguageParserWindow8RawBeam>(evaluator.evaluate(&input).unwrap()).unwrap();}ordinary+=start.elapsed().as_nanos();assert_eq!(value.encode().unwrap(),expected);
  let start=Instant::now();let mut frame=Vec::with_capacity(262144);frame.extend_from_slice(&bytes);for(_,evaluator)in &mut stages{let output=evaluator.evaluate(&frame).unwrap();let _=family.decode::<LanguageParserWindow8RawBeam>(output).unwrap();frame.clear();frame.extend_from_slice(output);}canonical+=start.elapsed().as_nanos();assert_eq!(frame,expected);
  let start=Instant::now();frame.clear();frame.extend_from_slice(&bytes);for(_,evaluator)in &mut stages{let output=evaluator.evaluate(&frame).unwrap();let _=family.decode_with_admitted_children::<LanguageParserWindow8RawBeam>(output,&refs,NativeChildAdmissionScope::maximum_scope_state_bytes()).unwrap();frame.clear();frame.extend_from_slice(output);}scoped+=start.elapsed().as_nanos();assert_eq!(frame,expected);
 }
 println!("PASS10actualepochs5originalSourcehops everyhopfreshNative original_reference_nanos={full_source} ordinary_encode_prepared_nanos={ordinary} canonical_prepared_nanos={canonical} canonical_child_scoped_nanos={scoped}");
}
