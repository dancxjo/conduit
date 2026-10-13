use conduit_plot::rust_binding::{PreparedNativeFamily,PreparedNativeFamilyLimits,PreparedNativeRustBinding,NativeChildAdmissionScope,AdmittedNativeChild};
use std::{rc::Rc,time::Instant};
fn main(){
 let limits=PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024};
 let start=Instant::now();let mut family=PreparedNativeFamily::prepare_with_child_admission(PREPARED_NATIVE_FAMILY_ROOTS,limits).unwrap();println!("prepare_nanos={} receipt={:?}",start.elapsed().as_nanos(),family.storage_receipt());
 let mut plain=0u128;let mut scoped=0u128;let mut reference=0u128;let mut cap_prep=0u128;let mut cap_storage=0usize;
 for epoch in 0..10 {
  let bytes=std::fs::read(format!("work/native-child-rank/epoch-{epoch}.bin")).unwrap();
  let view=conduit_core::validate_canonical_structured_value(&bytes).unwrap();let mut caps=Vec::with_capacity(4);
  let start=Instant::now();
  for field in ["candidate0","candidate1","candidate2","candidate3"]{
   let child=view.record_field(field).unwrap().unwrap();let mut frame=Vec::with_capacity(child.type_bytes().len()+child.value_node().len());frame.extend_from_slice(child.type_bytes());frame.extend_from_slice(child.value_node());
   let (_,cap)=family.decode_admitted::<LanguageParserWindow8RawHypothesis>(Rc::from(frame),usize::MAX).unwrap();cap_storage+=cap.storage_receipt().combined_bytes_bound;caps.push(cap);
  }cap_prep+=start.elapsed().as_nanos();let refs:Vec<&AdmittedNativeChild>=caps.iter().collect();
  let start=Instant::now();let expected=LanguageParserWindow8RawBeam::decode(&bytes).unwrap();reference+=start.elapsed().as_nanos();
  for _ in 0..20 {
   let start=Instant::now();let actual=family.decode::<LanguageParserWindow8RawBeam>(&bytes).unwrap();plain+=start.elapsed().as_nanos();assert_eq!(actual,expected);
   let start=Instant::now();let actual=family.decode_with_admitted_children::<LanguageParserWindow8RawBeam>(&bytes,&refs,NativeChildAdmissionScope::maximum_scope_state_bytes()).unwrap();scoped+=start.elapsed().as_nanos();assert_eq!(actual,expected);
  }
 }
 println!("PASS all10 retained actual epoch beam values; repeats=20 reference_once_nanos={reference} prepared_nanos={plain} child_scoped_nanos={scoped} capability_fresh_admission_nanos={cap_prep} cumulative_cap_combined_bound={cap_storage} scope_logical_bytes={}",NativeChildAdmissionScope::maximum_scope_state_bytes());
}
