use conduit_plot::*;
use conduit_core::*;
fn main(){
 let root=std::path::Path::new("semantics/language");
 let source=["types","identity","coverage","syntax","text_revision","lexical","lexical_proposer","parser","parser_mask","parser_window8","parser_window8_search","parser_window8_facts","parser_window8_proposal_features"].iter().map(|n|std::fs::read_to_string(root.join(format!("{n}.conduit"))).unwrap()).collect::<Vec<_>>().join("\n")+"\n"+&std::fs::read_to_string("work/lexical-proposer-next/proposal_window8_features_v2.conduit").unwrap();
 let checked=check_syntax_document(&parse_syntax_document(&source),&StartupCatalog::new()).unwrap();
 let expanded=expand_canonical_plot_for_authoring(&checked,"language-proposal-window8-v2-feature-values",&ProfileCatalog::new()).unwrap();
 let [entry]=expanded.expanded.gears[0].configuration.as_slice()else{panic!("one original program")};let ConfigurationValue::Text(hex)=&entry.value else{panic!("program")};let values=PortableExpressionProgram::from_canonical_hex(hex).unwrap();
 std::fs::write("work/lexical-proposer-next/proposal-v2-feature-values.hex",hex).unwrap();
 let out=std::path::Path::new(env!("OUT_DIR"));let context=PortableExpressionProgram::from_canonical_hex(&std::fs::read_to_string(out.join("proposal_window8_feature_context.hex")).unwrap()).unwrap();let old=PortableExpressionProgram::from_canonical_hex(&std::fs::read_to_string(out.join("proposal_window8_feature_values.hex")).unwrap()).unwrap();
 let guarded=checked.native_types.iter().find(|t|t.name=="LanguageParserProposalWindow8V2Features").unwrap();let mut composer=PreparedStructuredComposer::new(&guarded.value_type,262144).unwrap();let mut prepared=PreparedPortableExpressionEvaluator::new(&values).unwrap();
 for name in ["vocative","object"]{
  let input=std::fs::read(format!("work/lexical-proposer-next/new-feature-{name}.bin")).unwrap();let context=context.evaluate(&input).unwrap();let expected=values.evaluate(&context).unwrap();assert_eq!(prepared.evaluate(&context).unwrap(),expected);
  let view=validate_canonical_structured_value(&expected).unwrap();let frame=composer.record(&[view]).unwrap();let value=StructuredInfoValue::from_canonical_bytes(frame).unwrap();conduit_plot::rust_binding::validate_native_invariants(&value,&guarded.invariants).unwrap();
  let old=old.evaluate(&context).unwrap();let old=validate_canonical_structured_value(&old).unwrap();let vals=|v:ValidatedCanonicalStructuredValue<'_>|v.record_field("indices").unwrap().unwrap().collection_elements().unwrap().map(|x|u64::from_le_bytes(x.unwrap().value_node()[5..].try_into().unwrap())).collect::<Vec<_>>();
  let current=vals(view);let mut prior=vals(old);let pair=prior[2]-36;prior[2]=36+(pair/18).min(16)*17+(pair%18).min(16);for v in &mut prior[3..]{*v-=35;}assert_eq!(current,prior);assert!(current.iter().all(|v|*v<411));
  println!("exact {name} Source Reference/prepared +full27guard parity indices={current:?}");
 }
 println!("PASS checked{}Types; exact new Source expansion; unchanged unigrams/history/origin mechanics and27guards; no runtime model/generation/heldout claim",checked.native_types.len());
}
