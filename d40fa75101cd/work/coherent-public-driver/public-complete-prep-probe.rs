use std::sync::Arc;
use conduit_ai::native_profile::*;
use conduit_ai::operation_owners::native_profile::NativeProfileOperationFactory;
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
#[path="allocation_probe.rs"] mod allocation_probe;
#[global_allocator] static ALLOCATOR:allocation_probe::Allocator=allocation_probe::Allocator;
fn declarations_only(source:&str)->String{let mut active=false;let mut result=String::new();for line in source.lines(){if line.starts_with("type "){active=true;}if line.starts_with("plot "){active=false;}if active{result.push_str(line);result.push('\n');}}result}
fn prepare()->(Plan,Vec<Arc<PreparedNativeProfile>>,NativeProfileOperationFactory){
 let root=std::path::PathBuf::from(std::env::args().nth(1).unwrap());let plan:Plan=serde_json::from_slice(&std::fs::read(std::env::args().nth(2).unwrap()).unwrap()).unwrap();
 let mut parts=Vec::new();for(index,path)in["semantics/ai/fixed_numeric.conduit","semantics/ai/fixed_numeric_signal.conduit","semantics/speech/fargan_conditioning.conduit","semantics/speech/fargan_signal.conduit","semantics/speech/fargan_subframe.conduit","semantics/speech/fargan_model_identity.conduit","semantics/speech/fargan_epoch_contracts.conduit"].iter().enumerate(){let source=std::fs::read_to_string(root.join(path)).unwrap();parts.push(if index<2{source.lines().filter(|line|line.starts_with("type NumericFiniteF32 ")||line.starts_with("type NumericF32Vector")||line.starts_with("type NumericI16Vector")||line.starts_with("type NumericHistory")).collect::<Vec<_>>().join("\n")}else{declarations_only(&source)});}
 let definition=parts.join("\n");let feedback=std::fs::read_to_string(root.join("semantics/speech/fargan_epoch_feedback.conduit")).unwrap();let conditioning=std::fs::read_to_string(root.join("semantics/speech/fargan_conditioning_epoch_contracts.conduit")).unwrap();let mut profiles=Vec::new();
 for name in ["FarganFloatEpochInput","FarganFloatPhase1","FarganFloatPhase2","FarganFloatPhase3","FarganFloatEpochProposal","FarganPcm16EpochResult"]{profiles.push(Arc::new(PreparedNativeProfile::check_definition(&definition,name).unwrap()));}
 let d=definition.clone()+"\n"+&feedback;profiles.push(Arc::new(PreparedNativeProfile::check_definition(&d,"FarganSignalEpochFeedback").unwrap()));
 let d=definition.clone()+"\n"+&feedback+"\n"+&conditioning;for name in ["FarganConditioningInputEpoch","FarganConditioningProposalEpoch","FarganConditioningPendingHistory","FarganConditioningEpochFeedback","FarganSignalConditionEpoch"]{profiles.push(Arc::new(PreparedNativeProfile::check_definition(&d,name).unwrap()));}
 let d=definition.clone()+"\n"+&conditioning;profiles.push(Arc::new(PreparedNativeProfile::check_definition(&d,"FarganFeatureConditionEpoch").unwrap()));
 let feature=std::fs::read_to_string(root.join("semantics/speech/fargan_feature_epoch_contracts.conduit")).unwrap();let fd=definition.clone()+"\n"+&feedback+"\n"+&feature;let direct=fd.clone()+"\n"+&std::fs::read_to_string(root.join("semantics/speech/fargan_feature_direct16k_contracts.conduit")).unwrap();
 for name in ["FarganFeatureProposalEpoch","FarganFeaturePendingState","FarganFeatureEpochFeedback"]{profiles.push(Arc::new(PreparedNativeProfile::check_definition(&fd,name).unwrap()));}
 for name in ["FarganFeatureInputEpoch16k","FarganFeaturePcmEpoch16k"]{profiles.push(Arc::new(PreparedNativeProfile::check_definition(&direct,name).unwrap()));}
 let factory=NativeProfileOperationFactory::for_plan(&plan,&profiles).unwrap();(plan,profiles,factory)
}
fn main(){let((plan,profiles,factory),observed)=allocation_probe::observe(prepare);println!("complete immutable archived profile preparation requested={} peak={} live={} plan_heap={} profile_count={} factory={:?}",observed.requested_bytes,observed.peak_bytes,observed.live_bytes,plan_owned_heap_bytes(&plan).unwrap(),profiles.len(),factory.storage().unwrap());}
