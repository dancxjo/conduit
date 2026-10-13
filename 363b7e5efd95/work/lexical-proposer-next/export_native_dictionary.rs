#![allow(unused_imports,dead_code)]
use conduit_plot::rust_binding::NativeRustBinding;
// Exact version3 resource/profile custody. No graph or accuracy is inferred here.
use conduit_ai::{integer_categorical_step::PreparedCategoricalStep, *};
use conduit_core::*;
use conduit_data::*;
use conduit_language::{parser_model_selection::*, *};
use conduit_plot::rust_binding::BoundedSequence;
use std::sync::Arc;
pub fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|value| format!("{value:02x}")).collect()
}
pub fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::model(
        "TRAIN-plus-authored-lexical-overlay".into(),
        "4390281523cd89ef5e31dbccc9c2f9c049116dee3975fc6957f270ed8f195ebd".into(),
    )
    .unwrap()
}
pub fn lexical(bytes: &[u8]) -> LanguageLexicalProfile {
    let data: std::collections::BTreeMap<String, Vec<usize>> =
        serde_json::from_slice(bytes).unwrap();
    let pos = [
        LanguageLexicalPos::Adjective,
        LanguageLexicalPos::Adposition,
        LanguageLexicalPos::Adverb,
        LanguageLexicalPos::Auxiliary,
        LanguageLexicalPos::CoordinatingConjunction,
        LanguageLexicalPos::Determiner,
        LanguageLexicalPos::Interjection,
        LanguageLexicalPos::Noun,
        LanguageLexicalPos::Numeral,
        LanguageLexicalPos::Particle,
        LanguageLexicalPos::Pronoun,
        LanguageLexicalPos::ProperNoun,
        LanguageLexicalPos::Punctuation,
        LanguageLexicalPos::SubordinatingConjunction,
        LanguageLexicalPos::Symbol,
        LanguageLexicalPos::Verb,
        LanguageLexicalPos::Other,
    ];
    let entries = data.into_iter().map(|(word, codes)| {
        let candidates = codes.into_iter().map(|code| {
            LanguageLexicalCandidate::new(if word.len() <= 128 {word.clone()} else {"<unlemmatized>".into()}, BoundedSequence::new(), pos[code]).unwrap()
        });
        LanguageLexicalEntry::new(BoundedSequence::try_from_iter(candidates).unwrap(), word)
            .unwrap()
    });
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(),
        hex(semantic_digest("language/parser-lexical-profile@1", bytes)),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
    )
    .unwrap()
}

fn main(){
 let base=std::path::Path::new("work/lexical-proposer-next/train-dictionary-authored-overlay-v2");
 let out=std::path::Path::new("work/lexical-proposer-next/canonical-dictionary-overlay-v2");std::fs::create_dir(out).unwrap();
 let mut inputs=Vec::new();let mut rows=Vec::new();
 for index in 0..128{
  let path=base.join(format!("shard-{index:03}.json"));let bytes=std::fs::read(&path).unwrap();
  let data:serde_json::Value=serde_json::from_slice(&bytes).unwrap();
  let entries:std::collections::BTreeMap<String,Vec<usize>>=data["entries"].as_array().unwrap().iter().map(|e|(e["surface"].as_str().unwrap().to_owned(),e["pos_codes"].as_array().unwrap().iter().map(|v|v.as_u64().unwrap()as usize).collect())).collect();
  assert_eq!(entries.len(),64);let canonical=lexical(&serde_json::to_vec(&entries).unwrap()).encode().unwrap();
  std::fs::write(out.join(format!("shard-{index:03}.native.bin")),&canonical).unwrap();
  rows.push(serde_json::json!({"ordinal":index,"bytes":canonical.len(),"semantic_identity":hex(semantic_digest("language/lexical-dictionary-shard@1",&canonical))}));inputs.push(canonical);
 }
 use conduit_language::lexical_proposer_resource::*;
 use conduit_plot::rust_binding::PreparedNativeFamilyLimits;
 let dictionary=PreparedLexicalDictionary::prepare(inputs,"language/en",LexicalDictionaryLimits{maximum_shards:128,maximum_entries:8192,maximum_frame_bytes:262144,maximum_retained_bytes:300_000_000,maximum_preparation_peak_bytes:2_000_000_000,native:PreparedNativeFamilyLimits{maximum_types:64,maximum_laws_per_type:256,maximum_input_bytes:262144,maximum_retained_bytes:200_000_000,maximum_preparation_peak_bytes:1_000_000_000,maximum_conversion_requested_bytes:1_000_000_000}}).unwrap();
 let r=dictionary.storage_receipt();let manifest=serde_json::json!({"scope":"Offline canonical export and full Native dictionary preparation; not parser/model/Source execution acceptance","dictionary_identity":hex(dictionary.identity()),"lemma_policy":"surface unchanged when UTF8<=128bytes; otherwise explicit <unlemmatized>; no morphological truth assertion","receipt":{"shards":r.shards,"entries":r.entries,"retained_heap_bytes_bound":r.retained_heap_bytes_bound,"preparation_peak_heap_bytes_bound":r.preparation_peak_heap_bytes_bound,"lookup_requested_bytes_bound":r.lookup_requested_bytes_bound},"shards":rows});std::fs::write(out.join("manifest.json"),serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();println!("PASS full canonical128shards8192entries dictionary={}",hex(dictionary.identity()));
}
