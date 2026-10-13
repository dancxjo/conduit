include!("../../semantics/language/tests/lexical_proposer_policy.rs");
fn main(){
 let path="/home/dancxjo/conduit-4907-evaluation-identity/target/debug/build/conduit-language-a41c865d7c39adcc/out/lexical_token_proposal.hex";let hex=std::fs::read_to_string(path).unwrap();let raw:Vec<u8>=(0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect();
 let decode=conduit_plot::PortableExpressionProgram::canonical_decode_storage_bound(&raw).unwrap();let program=conduit_plot::PortableExpressionProgram::from_canonical_bytes_with_storage_limit(&raw,100_000_000).unwrap();
 assert_eq!(program.input_type.canonical_bytes().unwrap(),LanguageLexicalProposalQuery::PREPARED_DESCRIPTOR.type_bytes);assert_eq!(program.output_type.canonical_bytes().unwrap(),LanguageLexicalTokenProposal::PREPARED_DESCRIPTOR.type_bytes);
 let(mut prepared,receipt)=conduit_plot::PreparedPortableExpressionEvaluator::new_with_storage_limits(&program,100_000_000,200_000_000,100_000_000).unwrap();
 let query=LanguageLexicalProposalQuery::new(definition(),origin("unknown-policy"),None,token("<unknown>")).unwrap();let input=query.clone().encode().unwrap();let reference=program.evaluate(&input).unwrap();let output=prepared.evaluate(&input).unwrap();assert_eq!(reference,output);let mut family=family();let proposal:LanguageLexicalTokenProposal=family.decode(output).unwrap();assert_eq!(proposal.query(),&query);println!("PASS exact Source/prepared/family; decode_bound={decode} receipt={receipt:?}");
}
