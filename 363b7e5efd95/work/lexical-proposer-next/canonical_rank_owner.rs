//! WORK-only exact Source hop owner. No admission/commit/public Session authority.
use conduit_language::*;
use conduit_plot::{PortableExpressionProgram,PreparedPortableExpressionEvaluator,rust_binding::{NativeRustBinding,PreparedNativeRustBinding,PreparedNativeFamily,PreparedNativeFamilyLimits}};
pub struct RankOwner{evaluators:Vec<PreparedPortableExpressionEvaluator>,family:PreparedNativeFamily,frame:Vec<u8>,maximum:usize}
impl RankOwner{
 pub fn prepare(out:&std::path::Path)->Self{
  let family=PreparedNativeFamily::prepare(&[LanguageParserWindow8RawBeam::PREPARED_DESCRIPTOR],PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024}).unwrap();
  let mut retained=family.storage_receipt().retained_heap_bytes_bound;let mut evaluators=Vec::with_capacity(5);
  for name in ["window8_rank_0_1","window8_rank_2_3","window8_rank_0_2","window8_rank_1_3","window8_rank_1_2"]{
   let hex=std::fs::read_to_string(out.join(format!("{name}.hex"))).unwrap();let bytes=(0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect::<Vec<_>>();
   let decode=PortableExpressionProgram::canonical_decode_storage_bound(&bytes).unwrap();
   let program=PortableExpressionProgram::from_canonical_bytes_with_storage_limit(&bytes,decode).unwrap();
   let(evaluator,receipt)=PreparedPortableExpressionEvaluator::new_with_storage_limits(&program,decode,1024*1024*1024-retained,512*1024*1024-retained).unwrap();
   retained=retained.checked_add(receipt.retained_heap_bytes_bound).unwrap();evaluators.push(evaluator);
  }
  println!("rank_owner_component_retained_bound={retained}");
  Self{evaluators,family,frame:Vec::with_capacity(262144),maximum:262144}
 }
 pub fn rank(&mut self,beam:LanguageParserWindow8RawBeam)->LanguageParserWindow8RawBeam{
  let input=beam.encode().unwrap();assert!(input.len()<=self.maximum);self.frame.clear();self.frame.extend_from_slice(&input);
  let mut last=None;
  for evaluator in &mut self.evaluators{
   let bytes=evaluator.evaluate(&self.frame).unwrap();assert!(bytes.len()<=self.maximum);
   // Every original output is freshly readmitted with full immutable Type/laws.
   last=Some(self.family.decode::<LanguageParserWindow8RawBeam>(bytes).unwrap());
   self.frame.clear();self.frame.extend_from_slice(bytes);
  }
  last.unwrap()
 }
}
