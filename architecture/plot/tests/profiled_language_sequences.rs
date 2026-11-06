//! Use the current Language semantic Source without lowering lexical maxima.
use conduit_plot::{rust_binding::*, *};
#[path = "common/generated_rust.rs"]
mod generated_rust;
fn source() -> String {
    [
        include_str!("../../../semantics/language/identity.conduit"),
        include_str!("../../../semantics/language/types.conduit"),
        include_str!("../../../semantics/language/text_revision.conduit"),
        include_str!("../../../semantics/language/lexical.conduit"),
    ]
    .join("\n")
}
#[test]
fn actual_language_lexical_token_materializes_with_smaller_physical_profile() {
    let source = source();
    let d = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let token = d
        .native_types
        .iter()
        .find(|n| n.name == "LanguageLexicalToken")
        .unwrap();
    let schema = discover_prepared_storage_schema(&d.native_types, &token.identity).unwrap();
    assert!(
        schema.unsupported().is_empty(),
        "{:?}",
        schema.unsupported()
    );
    assert!(schema
        .sites()
        .iter()
        .any(|s| s.limit == PreparedStorageLimit::SequenceItems(4)));
    assert!(schema
        .sites()
        .iter()
        .any(|s| s.limit == PreparedStorageLimit::SequenceItems(8)));
    assert!(schema
        .sites()
        .iter()
        .any(|s| s.limit == PreparedStorageLimit::TextBytes(1024)));
    let generated = generate_rust_bindings_with_profiled_storage(
        &d.native_types,
        &RustBindingOptions::default(),
        &["LanguageLexicalToken"],
    )
    .unwrap();
    let exercise = format!(
        "\nconst SOURCE:&str={source:?};\nmod allocation_probe{{{}}}\n{}",
        include_str!("common/allocation_probe.rs"),
        EXERCISE
    );
    generated_rust::exercise(
        &generated.source,
        &exercise,
        3,
        "profiled-language-sequences",
    );
}
#[test]
fn current_dependency_query_materializes_without_claiming_execution_or_publication() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../semantics/language");
    let source = [
        "types.conduit",
        "identity.conduit",
        "text_revision.conduit",
        "lexical.conduit",
        "parser.conduit",
        "parser_beam.conduit",
        "parser_beam_commit.conduit",
        "dependency_commitment.conduit",
    ]
    .map(|file| std::fs::read_to_string(root.join(file)).unwrap())
    .join("\n");
    let startup = StartupCatalog::new();
    // Retain exact current Type declarations and their laws. This fixture does
    // not lower unrelated owner Plots or claim that any of them executed.
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty());
    let source = syntax
        .types
        .iter()
        .map(|t| &source[t.span.start..t.span.end])
        .collect::<Vec<_>>()
        .join("\n");
    let d = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let query = d
        .native_types
        .iter()
        .find(|n| n.name == "LanguageCommittedDependencyQuery")
        .unwrap();
    let schema = discover_prepared_storage_schema(&d.native_types, &query.identity).unwrap();
    assert!(
        schema.unsupported().is_empty(),
        "{:?}",
        schema.unsupported()
    );
    let generated = generate_rust_bindings_with_profiled_storage(
        &d.native_types,
        &RustBindingOptions::default(),
        &["LanguageLexicalToken", "LanguageCommittedDependencyQuery"],
    )
    .unwrap();
    let exercise = format!(
        "\nconst SOURCE:&str={source:?};\nmod allocation_probe{{{}}}\n{}\n{}",
        include_str!("common/allocation_probe.rs"),
        EXERCISE,
        QUERY_EXERCISE
    );
    generated_rust::exercise(&generated.source, &exercise, 4, "profiled-language-query");
}
const EXERCISE: &str = r#"
#[global_allocator]static ALLOCATOR:allocation_probe::Allocator=allocation_probe::Allocator;
#[cfg(test)]mod acceptance{
use super::*;use conduit_plot::{*,rust_binding::*};
fn doc()->CheckedSyntaxDocument{check_syntax_document(&parse_syntax_document(SOURCE),&StartupCatalog::new()).unwrap()}
fn bounds()->PreparedStorageBounds{PreparedStorageBounds{decode:PreparedDecodeBounds{canonical_bytes:65536,leaf_steps:256,text_bytes:65536},sequence_items:64,storage_bytes:262144}}
fn profile(d:&CheckedSyntaxDocument,count:usize,text:Option<usize>)->PreparedStorageProfile{
 let root=&d.native_types.iter().find(|n|n.name=="LanguageLexicalToken").unwrap().identity;
 let schema=discover_prepared_storage_schema(&d.native_types,root).unwrap();PreparedStorageProfile{root:root.clone(),capacities:schema.sites().iter().filter_map(|site|match site.limit{PreparedStorageLimit::SequenceItems(max)=>Some(PreparedStorageCapacity{path:site.path.clone(),limit:PreparedStorageLimit::SequenceItems(count.min(max))}),PreparedStorageLimit::TextBytes(max)=>text.map(|cap|PreparedStorageCapacity{path:site.path.clone(),limit:PreparedStorageLimit::TextBytes(cap.min(max))})}).collect()}
}
pub(super) fn token(candidates:usize,morphology:usize,feature:&str)->LanguageLexicalToken{
 let identity=||LinguisticTokenIdentity::new(0,LanguageTextId::new("text".into()).unwrap(),LanguageTextRevisionId::new("r1".into()).unwrap()).unwrap();
 let span=TextSpan::new(LinguisticOffsetBasis::UnicodeScalar,1,0,LanguageTextId::new("text".into()).unwrap(),LanguageTextRevisionId::new("r1".into()).unwrap()).unwrap();
 let candidate=||LanguageLexicalCandidate::new("bank".into(),BoundedSequence::try_from_iter((0..morphology).map(|_|LinguisticTokenFeature::new(feature.into(),"plural".into()).unwrap())).unwrap(),LanguageLexicalPos::Noun).unwrap();
 LanguageLexicalToken::new(BoundedSequence::try_from_iter((0..candidates).map(|_|candidate())).unwrap(),LinguisticTokenCategory::Word,LanguageLexicalCompleteness::Complete,identity(),Some(identity()),span,"bank".into()).unwrap()
}
fn admit<'d>(d:&'d CheckedSyntaxDocument,expected:&LanguageLexicalToken)->PreparedNativeAdmission<'d>{let mut a=PreparedNativeAdmission::new(d,&profile(d,0,None).root,PreparedNativeBounds{canonical_bytes:65536,scopes:256,invariants:256,contracts:256}).unwrap();a.begin(&expected.clone().encode().unwrap()).unwrap();while a.step().unwrap()!=PreparedNativeProgress::Admitted{}a}
fn quiet(c:allocation_probe::Observation){assert_eq!((c.allocations,c.reallocations,c.deallocations),(0,0,0));}
#[test]fn current_token_identity_text_laws_and_active_morphology_keep_zero_growth(){let d=doc();for (candidates,morphology)in [(0,0),(1,0),(2,2)]{
 let expected=token(candidates,morphology,"number");let a=admit(&d,&expected);let w=a.admitted().unwrap();let mut s=LanguageLexicalToken::prepare_profiled_storage(&d,&profile(&d,2,Some(16)),bounds()).unwrap();
 let ((actual,retained),c)=allocation_probe::observe(||{while s.step(&w).unwrap()!=PreparedDecodeProgress::Complete{}s.finish_retained(&w).map_err(|e|e.0).unwrap()});quiet(c);assert_eq!(actual,expected);assert!(retained.cost().text_bytes<65536);assert_eq!(actual.candidates().capacity(),4);if let Some(candidate)=actual.candidates().first(){assert_eq!(candidate.morphology().capacity(),8)}
}}
#[test]fn semantic_maximum_profile_is_not_silently_resized_to_fit_text_ceiling(){let d=doc();let p=profile(&d,8,None);assert!(LanguageLexicalToken::prepare_profiled_storage(&d,&p,bounds()).is_err()); // 4*8*2*1024 exceeds 65536 once remaining fields are counted.
 let expected=token(4,8,"number");let a=admit(&d,&expected);let w=a.admitted().unwrap();let mut s=LanguageLexicalToken::prepare_profiled_storage(&d,&profile(&d,2,Some(16)),bounds()).unwrap();assert_eq!(s.step(&w),Err(PreparedDecodeRefusal::Pressure));assert!(s.finish_retained(&w).is_err());
}
#[test]fn semantic_text_below_1024_but_above_physical_profile_returns_pressure(){let d=doc();let expected=token(1,1,"valid-but-above-sixteen");let a=admit(&d,&expected);let w=a.admitted().unwrap();let mut s=LanguageLexicalToken::prepare_profiled_storage(&d,&profile(&d,2,Some(16)),bounds()).unwrap();let ((reason,_retained),c)=allocation_probe::observe(||{loop{match s.step(&w){Ok(PreparedDecodeProgress::Pending)=>(),Ok(_)=>panic!("oversized feature published"),Err(e)=>{assert_eq!(e,PreparedDecodeRefusal::Pressure);break}}}s.finish_retained(&w).err().unwrap()});quiet(c);assert_eq!(reason,PreparedDecodeRefusal::Pressure);}
}
"#;

const QUERY_EXERCISE: &str = r#"
#[test]fn actual_checked_dependency_query_keeps_its_exact_native_laws_and_backing(){
 use conduit_plot::{*,rust_binding::*};
 let d=check_syntax_document(&parse_syntax_document(SOURCE),&StartupCatalog::new()).unwrap();
 let text=LanguageTextId::new("text".into()).unwrap();let revision=LanguageTextRevisionId::new("r1".into()).unwrap();let analysis=LanguageAnalysisRevisionId::new("a1".into()).unwrap();
 let basis=LanguageParserBasis::new(analysis.clone(),revision.clone(),text.clone()).unwrap();
 let root=LanguageParserRelation::new(LanguageUniversalDependencyRelation::Root,LanguageParserSubtype::new("".into()).unwrap()).unwrap();
 let other=LanguageParserRelation::new(LanguageUniversalDependencyRelation::Dep,LanguageParserSubtype::new("".into()).unwrap()).unwrap();
 let state=LanguageParserState::new(basis.clone(),0,2,[4,5,5,5,4],root,other.clone(),other.clone(),other,[4,0,4,4,4],1,1).unwrap();
 let candidate=|active,id|LanguageParserHypothesis::new(active,id,0,state.clone()).unwrap();
 let beam=LanguageParserAdmittedBeam::new(basis.clone(),candidate(true,1),candidate(true,2),candidate(false,3),candidate(false,4)).unwrap();
 let commitment=LanguageParserBeamCommitRequest::new(beam,false,LanguageParserRawFrontier::new(basis,0,1).unwrap(),false,1).unwrap();
 let token=acceptance::token(1,1,"number");
 let arc=LanguageDependencyArc::new(LanguageAnalysisTokenRef::new(analysis,token.identity().clone()).unwrap(),LanguageDependencyHead::Root,LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Root,None).unwrap()).unwrap();
 let provenance=LinguisticDerivationProvenance::DeterministicRule(LinguisticDerivationProvenanceDeterministicRule{implementation:"prepared-query-fixture".into(),revision:"1".into()});
 let source=LanguageTextRevision::new(LanguageTextFinality::Final,LanguageText::new(text,LanguageId::new("language/en".into()).unwrap(),revision,"bank".into()).unwrap(),None,provenance,0,None).unwrap();
 let expected=LanguageCommittedDependencyQuery::new(arc,commitment,4,source,token).unwrap();
 let root=&d.native_types.iter().find(|n|n.name=="LanguageCommittedDependencyQuery").unwrap().identity;let schema=discover_prepared_storage_schema(&d.native_types,root).unwrap();
 let profile=PreparedStorageProfile{root:root.clone(),capacities:schema.sites().iter().map(|site|PreparedStorageCapacity{path:site.path.clone(),limit:match site.limit{PreparedStorageLimit::SequenceItems(max)=>PreparedStorageLimit::SequenceItems(max.min(2)),PreparedStorageLimit::TextBytes(max)=>PreparedStorageLimit::TextBytes(max.min(32))}}).collect()};
 let mut storage=LanguageCommittedDependencyQuery::prepare_profiled_storage(&d,&profile,PreparedStorageBounds{decode:PreparedDecodeBounds{canonical_bytes:65536,leaf_steps:4096,text_bytes:65536},sequence_items:64,storage_bytes:1048576}).unwrap();
 let bytes=expected.clone().encode().unwrap();assert!(bytes.len()<=65536);
 let mut admission=PreparedNativeAdmission::new(&d,root,PreparedNativeBounds{canonical_bytes:65536,scopes:2048,invariants:2048,contracts:2048}).unwrap();admission.begin(&bytes).unwrap();while admission.step().unwrap()!=PreparedNativeProgress::Admitted{}let witness=admission.admitted().unwrap();
 let ((actual,retained),growth)=allocation_probe::observe(||{while storage.step(&witness).unwrap()!=PreparedDecodeProgress::Complete{}storage.finish_retained(&witness).map_err(|e|e.0).unwrap()});
 assert_eq!((growth.allocations,growth.reallocations,growth.deallocations),(0,0,0));assert_eq!(actual,expected);assert!(retained.cost().maximum_steps<=4096);
 // A decoded query is immutable data. No commitment operation, accepted result,
 // Current authority, journal adoption or dependency publication is asserted.
}
"#;
