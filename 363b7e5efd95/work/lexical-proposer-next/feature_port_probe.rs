include!("../../semantics/language/tests/lexical_proposer_revision.rs");
use conduit_plot::*;
use conduit_core::*;
use std::path::Path;
fn view(b:&[u8])->ValidatedCanonicalStructuredValue<'_>{validate_canonical_structured_value(b).unwrap()}
fn field<'a>(v:ValidatedCanonicalStructuredValue<'a>,n:&str)->ValidatedCanonicalStructuredValue<'a>{v.record_field(n).unwrap().unwrap()}
fn bytes(v:ValidatedCanonicalStructuredValue<'_>)->Vec<u8>{[v.type_bytes(),v.value_node()].concat()}
fn field_type(t:&StructuredInfoType,name:&str)->StructuredInfoType{match t.shape(){StructuredInfoTypeShape::Record{fields,..}=>fields.iter().find(|f|f.name()==name).unwrap().value_type().clone(),_=>panic!("record")}}
fn program(checked:&CheckedSyntaxDocument,name:&str)->PortableExpressionProgram{
 println!("expand={name}");
 let expanded=expand_canonical_plot_for_authoring(checked,name,&ProfileCatalog::new()).unwrap();
 let [entry]=expanded.expanded.gears[0].configuration.as_slice()else{panic!("one program")};
 let ConfigurationValue::Text(hex)=&entry.value else{panic!("program text")};
 let b:Vec<u8>=hex.as_bytes().chunks_exact(2).map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect();
 std::fs::write(format!("work/lexical-proposer-next/{name}.bin"),&b).unwrap();
 PortableExpressionProgram::from_canonical_bytes(&b).unwrap()
}
fn main(){
 let root=Path::new("semantics/language");
 let source=["types","identity","coverage","syntax","text_revision","lexical","lexical_proposer","parser","parser_mask","parser_window8","parser_window8_search","parser_window8_facts"].iter().map(|n|std::fs::read_to_string(root.join(format!("{n}.conduit"))).unwrap()).collect::<Vec<_>>().join("\n")+"\n"+&std::fs::read_to_string("work/lexical-proposer-next/proposal_window8_features.conduit").unwrap();
 let checked=check_syntax_document(&parse_syntax_document(&source),&StartupCatalog::new()).unwrap();
 let ty=|name:&str|checked.native_types.iter().find(|t|t.name==name).unwrap().value_type.clone();
 let origins=ty("LanguageParserProposalWindow8RawOrigins");let origin_guard=ty("LanguageParserProposalWindow8Origins");let query=ty("LanguageParserProposalWindow8FeatureQuery");
 let origin_program=program(&checked,"language-proposal-window8-origins");
 let context=program(&checked,"language-proposal-window8-feature-context");let values=program(&checked,"language-proposal-window8-feature-values");let guarded=ty("LanguageParserProposalWindow8Features");
 let mut cp=PreparedPortableExpressionEvaluator::new(&context).unwrap();let mut vp=PreparedPortableExpressionEvaluator::new(&values).unwrap();let mut gp=PreparedStructuredComposer::new(&guarded,262144).unwrap();

 let mut revision_owner=lexical_proposer_port::token_producer::revision::PreparedRevisionProducer::prepare(PreparedTokenProducer::prepare(port(),producer_limits()).unwrap(),revision_limits()).unwrap();
 let revision=original_revision(0,None,"record glorp!",Some(12),false).encode().unwrap();
 let admitted=revision_owner.propose(&revision,None,0).unwrap();
 let proposed=LanguageLexicalProposedTape::decode(admitted.canonical_proposed_tape()).unwrap();
 let lexical=LanguageParserWindow8Lexical::new(proposed.tape().clone(),3).unwrap().encode().unwrap();
 let origin_query_ty=ty("LanguageParserProposalWindow8OriginQuery");
 let mut origin_query=PreparedStructuredComposer::new(&origin_query_ty,262144).unwrap();
 let input=origin_query.record(&[view(&lexical),view(admitted.canonical_proposed_tape())]).unwrap();
 let origin_native=checked.native_types.iter().find(|t|t.name=="LanguageParserProposalWindow8OriginQuery").unwrap();
 conduit_plot::rust_binding::validate_native_invariants(&StructuredInfoValue::from_canonical_bytes(input).unwrap(),&origin_native.invariants).unwrap();
 let expected=origin_program.evaluate(input).unwrap();let mut prepared_origins=PreparedPortableExpressionEvaluator::new(&origin_program).unwrap();assert_eq!(prepared_origins.evaluate(input).unwrap(),expected);
 let observed:Vec<u64>=field(view(&expected),"origins").collection_elements().unwrap().map(|v|u64::from_le_bytes(v.unwrap().value_node()[5..].try_into().unwrap())).collect();assert_eq!(observed,[0,1,1,2,2,2,2,2]);
 std::fs::write("work/lexical-proposer-next/actual-origin-query.bin",input).unwrap();std::fs::write("work/lexical-proposer-next/actual-origin-output.bin",&expected).unwrap();println!("actual original revision/token Source→origin projection Reference/prepared parity {observed:?}; full lexical tape equality law passed");
 let mut result=Vec::new();
 for name in ["vocative","object"]{
  let raw=std::fs::read(format!("work/lexical-proposer-next/collision-{name}.bin")).unwrap();let projection=field(view(&raw),"projection");
  // This probe constructs reviewed origin observations for both legal fixtures.
  // Actual revision-origin Source port custody is a separate readiness gate.
  let mut origin_values=field_type(&origins,"origins").canonical_bytes().unwrap();origin_values.push(1);origin_values.extend_from_slice(&8u32.to_le_bytes());
  for i in 0..8{origin_values.push(0);origin_values.extend_from_slice(&8u32.to_le_bytes());origin_values.extend_from_slice(&(if i<5{0u64}else{2}).to_le_bytes());}
  let mut oc=PreparedStructuredComposer::new(&origins,262144).unwrap();let origin=oc.record(&[view(&origin_values),field(projection,"profile_identity"),field(projection,"sequence"),field(projection,"source_revision"),field(projection,"text"),field(projection,"token_count")]).unwrap();
  let mut og=PreparedStructuredComposer::new(&origin_guard,262144).unwrap();let origin=og.record(&[view(origin)]).unwrap();let og_native=checked.native_types.iter().find(|t|t.name=="LanguageParserProposalWindow8Origins").unwrap();conduit_plot::rust_binding::validate_native_invariants(&StructuredInfoValue::from_canonical_bytes(origin).unwrap(),&og_native.invariants).unwrap();
  let mut qc=PreparedStructuredComposer::new(&query,262144).unwrap();let input=qc.record(&[view(origin),view(&raw)]).unwrap();
  let q=StructuredInfoValue::from_canonical_bytes(input).unwrap();let native=checked.native_types.iter().find(|t|t.name=="LanguageParserProposalWindow8FeatureQuery").unwrap();
  conduit_plot::rust_binding::validate_native_invariants(&q,&native.invariants).unwrap();
  let expected_context=context.evaluate(input).unwrap();let expected=values.evaluate(&expected_context).unwrap();let expected_guarded=gp.record(&[view(&expected)]).unwrap().to_vec();
  let actual_context=cp.evaluate(input).unwrap();let actual=vp.evaluate(actual_context).unwrap();assert_eq!(actual,expected);assert_eq!(gp.record(&[view(actual)]).unwrap(),expected_guarded);
  let indices=field(view(actual),"indices");let collected:Vec<u64>=indices.collection_elements().unwrap().map(|v|u64::from_le_bytes(v.unwrap().value_node()[5..].try_into().unwrap())).collect();assert_eq!(collected.len(),27);assert!(collected.iter().all(|v|*v<446));
  let guarded_value=StructuredInfoValue::from_canonical_bytes(&expected_guarded).unwrap();let guard=checked.native_types.iter().find(|t|t.name=="LanguageParserProposalWindow8Features").unwrap();conduit_plot::rust_binding::validate_native_invariants(&guarded_value,&guard.invariants).unwrap();
  std::fs::write(format!("work/lexical-proposer-next/new-feature-{name}.bin"),bytes(view(input))).unwrap();println!("constructed={name} last_two={:?} all27={collected:?}",&collected[25..]);result.push(collected);
 }
 assert_eq!(&result[0][..25],&result[1][..25]);assert_ne!(result[0][25],result[1][25]);assert_eq!(result[0][26],443);assert_eq!(result[1][26],443);
 println!("PASS exact Source Reference/prepared parity, all27complete output guards, collision separated by selected previous POS; constructed reviewed-origin fixtures only; no model inference or original origin-port custody claim");
}
