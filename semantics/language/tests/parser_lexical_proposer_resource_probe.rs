extern crate alloc;
use conduit_language::*;
use conduit_plot::rust_binding::{NativeRustBinding,PreparedNativeFamilyLimits,BoundedSequence};
#[path="/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/src/lexical_proposer_resource.rs"]
mod resource;
fn native()->PreparedNativeFamilyLimits{PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:200_000_000,maximum_preparation_peak_bytes:1_000_000_000,maximum_conversion_requested_bytes:1_000_000_000}}
fn limits()->resource::LexicalDictionaryLimits{resource::LexicalDictionaryLimits{maximum_shards:128,maximum_entries:8192,maximum_frame_bytes:262144,maximum_retained_bytes:300_000_000,maximum_preparation_peak_bytes:2_000_000_000,native:native()}}
fn shard(word:&str,lang:&str)->Vec<u8>{LanguageLexicalProfile::new(BoundedSequence::try_from_iter([LanguageLexicalEntry::new(BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(word.into(),BoundedSequence::new(),LanguageLexicalPos::Noun).unwrap()]).unwrap(),word.into()).unwrap()]).unwrap(),format!("shard/{word}"),LanguageId::new(lang.into()).unwrap(),LinguisticDerivationProvenance::deterministic_rule("reviewed-test".into(),"1".into()).unwrap()).unwrap().encode().unwrap()}
#[test]
fn complete_shard_lookup_foreign_duplicate_and_quota_refusals(){
 let inputs=vec![shard("record","language/en"),shard("old","language/en")];
 let mut p=resource::PreparedLexicalDictionary::prepare(inputs.clone(),"language/en",limits()).unwrap();
 let r=p.storage_receipt();assert_eq!(r.shards,2);assert_eq!(r.entries,2);assert!(r.preparation_peak_heap_bytes_bound>=r.retained_heap_bytes_bound);assert!(r.lookup_requested_bytes_bound>0);
 let lookup=p.lookup("old").unwrap().unwrap();assert_eq!(lookup.entry.surface(),"old");assert_eq!(lookup.shard,1);assert_eq!(lookup.ordinal,0);assert_eq!(lookup.dictionary_identity,p.identity());assert_ne!(lookup.dictionary_identity,lookup.shard_identity);assert!(p.lookup("unlisted").unwrap().is_none());
 assert!(matches!(resource::PreparedLexicalDictionary::prepare(vec![inputs[0].clone(),inputs[0].clone()],"language/en",limits()),Err(resource::LexicalDictionaryRefusal::DuplicateSurface)));
 assert!(matches!(resource::PreparedLexicalDictionary::prepare(vec![shard("record","language/fr")],"language/en",limits()),Err(resource::LexicalDictionaryRefusal::Language)));
 let mut tiny=limits();tiny.maximum_retained_bytes=r.retained_heap_bytes_bound-1;assert!(resource::PreparedLexicalDictionary::prepare(inputs.clone(),"language/en",tiny).is_err());
 let mut tiny=limits();tiny.maximum_preparation_peak_bytes=r.preparation_peak_heap_bytes_bound-1;assert!(resource::PreparedLexicalDictionary::prepare(inputs.clone(),"language/en",tiny).is_err());
 let mut malformed=inputs;malformed[0].push(0);assert!(resource::PreparedLexicalDictionary::prepare(malformed,"language/en",limits()).is_err());
}
