#![allow(dead_code)]
include!("../../semantics/language/tests/lexical_proposer_revision.rs");
#[path="../../semantics/language/tests/common/parser_model_resource.rs"] mod model_resource;
use conduit_ai::{ModelArtifact,ModelSignature};
use conduit_core::*;
use std::{path::Path,sync::Arc};
use lexical_proposer_port::token_producer::model_definition::*;
fn main(){
 let archive=Path::new("semantics/language/training/ewt_proposal_v1");let expected=archive.join("proposal27-expected-metadata-v1");
 let shards=(0..128).map(|n|std::fs::read(archive.join(format!("canonical-dictionary-overlay-v2/shard-{n:03}.native.bin"))).unwrap()).collect();
 let dictionary=resource::PreparedLexicalDictionary::prepare(shards,"language/en",limits()).unwrap();
 assert_eq!(dictionary.identity().iter().map(|v|format!("{v:02x}")).collect::<String>(),"12458ed85a62ff19a794a8f79e24ff8c7e76e3975392379ed3ba9ed5c9ec89fe");
 let proposer=std::fs::read(expected.join("proposer-definition.native.bin")).unwrap();
 let port=PreparedLexicalProposerPort::prepare(dictionary,&proposer,LexicalProposerPortLimits{maximum_frame_bytes:262144,maximum_retained_bytes:300_000_000,maximum_preparation_peak_bytes:2_000_000_000,maximum_admission_peak_bytes:2_000_000_000,maximum_response_requested_bytes:1_000_000_000,native:native()}).unwrap();
 let owner=lexical_proposer_port::token_producer::revision::PreparedRevisionProducer::prepare(PreparedTokenProducer::prepare(port,producer_limits()).unwrap(),revision_limits()).unwrap();
 let signature=ModelSignature::decode(&std::fs::read(expected.join("signature.native.bin")).unwrap()).unwrap();
 let reference=BoundedResourceRef::decode(&std::fs::read(expected.join("artifact-reference.bin")).unwrap()).unwrap();
 let artifact=ModelArtifact{architecture_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_ARCHITECTURE.into(),format_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_FORMAT.into(),precision_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_PRECISION.into(),state_schema_version:1,signature_identity:signature.semantic_digest().unwrap(),content:reference};
 assert_eq!(format!("{artifact:#?}"),std::fs::read_to_string(expected.join("artifact-full.debug.txt")).unwrap());
 let indices=StructuredInfoType::from_canonical_bytes(&std::fs::read(expected.join("numeric-indices.type.bin")).unwrap()).unwrap();
 let scores=StructuredInfoType::from_canonical_bytes(&std::fs::read(expected.join("numeric-scores.type.bin")).unwrap()).unwrap();
 let source=std::fs::read("semantics/language/parser_window8.conduit").unwrap();let search=std::fs::read("semantics/language/parser_window8_search.conduit").unwrap();let feature=std::fs::read("semantics/language/parser_window8_proposal_features.conduit").unwrap();
 let contracts=conduit_language::parser_model_selection::ParserSourceModelContract{feature_contract:semantic_digest("language/parser-proposal-window8-scorer-encoding@1",&feature),availability_contract:semantic_digest("language/parser-window8-available@3",&source),action_contract:semantic_digest("language/parser-window8-action@3",&source),joint_choice_contract:semantic_digest("language/parser-window8-choice-search@3",&search),numeric_indices_contract:indices.semantic_digest().unwrap(),numeric_scores_contract:scores.semantic_digest().unwrap()};
 let training=std::fs::read(archive.join("checkpoint-files.json")).unwrap();
 let definition=Arc::new(ProposalModelDefinition::new(&owner,"TRAIN+22-reviewed-proposal-candidate1".into(),"selected-history-origin/446-27-76@1".into(),contracts.clone(),artifact,signature.clone(),(446,76,27),1_000_000,training.into_boxed_slice(),262144).unwrap());
 let weights=std::fs::read(archive.join("proposal-446-training-v2/proposal_window8.i16")).unwrap();
 let scorer=model_resource::categorical(weights,signature,1);
 let selected=PreparedProposalModelSelection::prepare(&owner,scorer.clone(),definition,&contracts).unwrap();
 assert_eq!(selected.categorical().dimensions(),(446,76,27));
 println!("PASS complete8192dictionary/originalproposer/independent fullartifact+signature/actual Source contracts/modelselection; numerical resource {:?}; revision {:?}; no inference/heldout/publicSession claim",scorer.storage_receipt(),owner.storage_receipt());
}
