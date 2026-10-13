from pathlib import Path
import hashlib
repo=Path('/home/dancxjo/conduit-4898-pair-storage');old=Path('/home/dancxjo/conduit-4898-common-greeting');proj=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
paths=['semantics/ai/fixed_numeric.conduit','semantics/ai/fixed_numeric_signal.conduit']+['semantics/speech/'+n+'.conduit' for n in ['fargan_conditioning','fargan_signal','fargan_subframe','fargan_model_identity','fargan_epoch_contracts','fargan_epoch_feedback','fargan_conditioning_epoch_contracts','fargan_feature_epoch_contracts','fargan_feature_direct16k_contracts']]
def pin(p):
 b=p.read_bytes();return f'Pin {{ length: {len(b)}, sha256: {list(hashlib.sha256(b).digest())} }}'
s='''//! Exact archived FARGAN native-profile preparation capability.
//! This is an immutable host preparation profile, not a generic checker bound.
//! Reservation covers new requested payload allocations/peak during this fixed
//! decode/check/verify/factory algorithm. Existing full input Arc allocations are
//! charged separately. Allocator bookkeeping/stack/whole runtime are not claimed.
use alloc::{sync::Arc, string::String, vec::Vec};
use conduit_ai::{native_profile::PreparedNativeProfile, operation_owners::native_profile::NativeProfileOperationFactory};
use conduit_core::Plan;
use sha2::{Digest,Sha256};
#[derive(Clone,Copy)] struct Pin { length: usize, sha256: [u8;32] }
impl Pin { fn accepts(self, bytes:&[u8])->bool { bytes.len()==self.length && Sha256::digest(bytes).as_slice()==self.sha256 } }
pub const DECLARATION_PATHS: [&str;11] = PATHS;
const DECLARATIONS: [Pin;11] = PINS;
const PLAN: Pin = PLANPIN;
const SOURCE: Pin = SOURCEPIN;
const RESOURCES:[Pin;3] = RESOURCEPINS;
/// Reviewed finite immutable-profile reservation. Unsupported bytes refuse before
/// any parser/checker/factory call. Upstream input construction is separate.
pub const PREPARATION_REQUESTED_BYTES:usize=320*1024*1024;
pub const PREPARATION_PEAK_BYTES:usize=96*1024*1024;
#[derive(Clone,Copy,Debug)] pub struct Limits { pub preparation_requested_bytes:usize, pub preparation_peak_bytes:usize, pub existing_input_bytes:usize }
#[derive(Clone,Copy,Debug,PartialEq,Eq)] pub enum Refusal { Capacity, ForeignPlan, ForeignSource, ForeignDeclaration(usize), ForeignResource(usize), Decode, Source, Factory }
pub struct Inputs { pub plan:Arc<[u8]>, pub source:Arc<str>, pub declarations:[Arc<str>;11], pub resources:[Arc<[u8]>;3] }
impl Inputs {
 pub fn owned_payload_bytes(&self)->usize { self.plan.len()+self.source.len()+self.declarations.iter().map(|v|v.len()).sum::<usize>()+self.resources.iter().map(|v|v.len()).sum::<usize>() }
}
#[derive(Clone,Copy,Debug)] pub struct Receipt { pub preparation_requested_bytes_bound:usize,pub preparation_peak_bytes_bound:usize,pub original_input_payload_bytes:usize }
pub struct PreparedNativeProfiles { inputs:Inputs, plan:Arc<Plan>, profiles:Vec<Arc<PreparedNativeProfile>>, factory:NativeProfileOperationFactory, receipt:Receipt }
impl PreparedNativeProfiles {
 pub fn inputs(&self)->&Inputs { &self.inputs }
 pub fn plan(&self)->&Arc<Plan> { &self.plan }
 pub fn profiles(&self)->&[Arc<PreparedNativeProfile>] { &self.profiles }
 pub fn factory(&self)->&NativeProfileOperationFactory { &self.factory }
 pub fn receipt(&self)->Receipt { self.receipt }
 pub fn prepare(inputs:Inputs, limits:Limits)->Result<Self,Refusal> {
  let input_bytes=inputs.owned_payload_bytes();
  if limits.preparation_requested_bytes<PREPARATION_REQUESTED_BYTES || limits.preparation_peak_bytes<PREPARATION_PEAK_BYTES || limits.existing_input_bytes<input_bytes { return Err(Refusal::Capacity); }
  if !PLAN.accepts(&inputs.plan) { return Err(Refusal::ForeignPlan); }
  if !SOURCE.accepts(inputs.source.as_bytes()) { return Err(Refusal::ForeignSource); }
  for (i,(pin,material)) in DECLARATIONS.iter().zip(&inputs.declarations).enumerate() { if !pin.accepts(material.as_bytes()) { return Err(Refusal::ForeignDeclaration(i)); } }
  for (i,(pin,material)) in RESOURCES.iter().zip(&inputs.resources).enumerate() { if !pin.accepts(material) { return Err(Refusal::ForeignResource(i)); } }
  let plan:Plan=serde_json::from_slice(&inputs.plan).map_err(|_|Refusal::Decode)?;
  let parts:Vec<String>=inputs.declarations[..7].iter().enumerate().map(|(index,source)| if index<2 { source.lines().filter(|line|line.starts_with("type NumericFiniteF32 ")||line.starts_with("type NumericF32Vector")||line.starts_with("type NumericI16Vector")||line.starts_with("type NumericHistory")).collect::<Vec<_>>().join("\\n") }else{declarations_only(source)}).collect();
  let definition=parts.join("\\n");let feedback=&inputs.declarations[7];let conditioning=&inputs.declarations[8];let mut profiles=Vec::new();
  for name in ["FarganFloatEpochInput","FarganFloatPhase1","FarganFloatPhase2","FarganFloatPhase3","FarganFloatEpochProposal","FarganPcm16EpochResult"] { profiles.push(Arc::new(PreparedNativeProfile::check_definition(&definition,name).map_err(|_|Refusal::Source)?)); }
  let d=definition.clone()+"\\n"+feedback;profiles.push(Arc::new(PreparedNativeProfile::check_definition(&d,"FarganSignalEpochFeedback").map_err(|_|Refusal::Source)?));
  let d=definition.clone()+"\\n"+feedback+"\\n"+conditioning;
  for name in ["FarganConditioningInputEpoch","FarganConditioningProposalEpoch","FarganConditioningPendingHistory","FarganConditioningEpochFeedback","FarganSignalConditionEpoch"] { profiles.push(Arc::new(PreparedNativeProfile::check_definition(&d,name).map_err(|_|Refusal::Source)?)); }
  let d=definition.clone()+"\\n"+conditioning;profiles.push(Arc::new(PreparedNativeProfile::check_definition(&d,"FarganFeatureConditionEpoch").map_err(|_|Refusal::Source)?));
  let fd=definition+"\\n"+feedback+"\\n"+&inputs.declarations[9];let direct=fd.clone()+"\\n"+&inputs.declarations[10];
  for name in ["FarganFeatureProposalEpoch","FarganFeaturePendingState","FarganFeatureEpochFeedback"] { profiles.push(Arc::new(PreparedNativeProfile::check_definition(&fd,name).map_err(|_|Refusal::Source)?)); }
  for name in ["FarganFeatureInputEpoch16k","FarganFeaturePcmEpoch16k"] { profiles.push(Arc::new(PreparedNativeProfile::check_definition(&direct,name).map_err(|_|Refusal::Source)?)); }
  let factory=NativeProfileOperationFactory::for_plan(&plan,&profiles).map_err(|_|Refusal::Factory)?;
  if conduit_plot::syntax_source_document_identity(&inputs.source)!=plan.source_document_id {return Err(Refusal::ForeignSource);}
  Ok(Self { inputs, plan:Arc::new(plan), profiles, factory, receipt:Receipt { preparation_requested_bytes_bound:PREPARATION_REQUESTED_BYTES, preparation_peak_bytes_bound:PREPARATION_PEAK_BYTES, original_input_payload_bytes:input_bytes } })
 }
}
fn declarations_only(source:&str)->String { let mut active=false;let mut result=String::new();for line in source.lines(){if line.starts_with("type "){active=true;}if line.starts_with("plot "){active=false;}if active{result.push_str(line);result.push('\\n');}}result }
'''
s=s.replace('= PATHS;', '= '+repr(paths).replace(chr(39),chr(34))+';').replace('= PINS;', '= ['+','.join(pin(old/p) for p in paths)+'];').replace('= PLANPIN;', '= '+pin(proj/'outputs/committed-common-fargan-greeting/sealed-epoch-plan.json')+';').replace('= SOURCEPIN;', '= '+pin(proj/'outputs/committed-common-fargan-greeting/checked-epoch-source.conduit')+';').replace('= RESOURCEPINS;', '= ['+','.join(pin(proj/'work/fargan/resources'/n) for n in ['f32.bin','compact.bin','manifest.json'])+'];')
(repo/'targets/conduitos/src/fargan_native_profile_preparation.rs').write_text(s)
