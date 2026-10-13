from pathlib import Path
p=Path('semantics/language/parser_joint.conduit');s=p.read_text()
old='type LanguageParserJointPosContext = {\n    query: LanguageParserJointChoiceQuery\n    pos: collection U64 = 4\n}'
extra='type LanguageParserJointTokenContext = {\n    query: LanguageParserJointChoiceQuery\n'+''.join(f'    token{i}: LanguageLexicalToken\n' for i in range(4))+'}\n'
extra+='type LanguageParserJointCandidateContext = {\n    query: LanguageParserJointChoiceQuery\n'+''.join(f'    candidate{i}: LanguageLexicalCandidate\n' for i in range(4))+'}\n'
new=extra+old[:-2]+''.join(f'\n    candidate{i}: LanguageLexicalCandidate' for i in range(4))+'\n}'
s=s.replace(old,new)
plots='plot language-parser-joint-tokens (\n    query: LanguageParserJointChoiceQuery...| >> context: LanguageParserJointTokenContext...|\n) = { query: ., '+', '.join(f'token{i}: sequence/at(.lexical.tape.tokens, (.lexical.token_count <= {i} ? 0 : {i}))' for i in range(4))+' }\n'
plots+='plot language-parser-joint-candidates (\n    tokens: LanguageParserJointTokenContext...| >> context: LanguageParserJointCandidateContext...|\n) = { query: .query, '+', '.join(f'candidate{i}: sequence/at(.token{i}.candidates, (.query.lexical.token_count <= {i} ? 0 : .query.choices.{i}))' for i in range(4))+' }\n'
s=s.replace('plot language-parser-joint-pos-begin (',plots+'plot language-parser-joint-pos-begin (')
s=s.replace('query: LanguageParserJointChoiceQuery...| >> context: LanguageParserJointPosContext...|\n) = { query: ., pos: [16,16,16,16] }','candidates: LanguageParserJointCandidateContext...| >> context: LanguageParserJointPosContext...|\n) = { query: .query, pos: [16,16,16,16], '+', '.join(f'candidate{i}: .candidate{i}' for i in range(4))+' }')
for i in range(4):
 s=s.replace(f'sequence/at(sequence/at(.query.lexical.tape.tokens, {i}).candidates, .query.choices.{i}).pos',f'.candidate{i}.pos')
# Preserve selected candidate values across each simple POS mapping stage.
s=s.replace(') = { query: .query, pos: [',') = { query: .query, '+', '.join(f'candidate{i}: .candidate{i}' for i in range(4))+', pos: [')
s=s.replace('    query >> language-parser-joint-pos-begin()', '    query >> language-parser-joint-tokens() >> language-parser-joint-candidates() >> language-parser-joint-pos-begin()')
p.write_text(s)
p=Path('semantics/language/src/lib.rs');p.write_text(p.read_text()+'\npub use generated::{LanguageParserJointTokenContext, LanguageParserJointCandidateContext};\n')
p=Path('semantics/language/src/parser.rs');s=p.read_text().replace('    alloc::vec![','    alloc::vec![\n        ("LanguageParserJointTokenContext", crate::LanguageParserJointTokenContext::semantic_type().expect("checked Language Type")),\n        ("LanguageParserJointCandidateContext", crate::LanguageParserJointCandidateContext::semantic_type().expect("checked Language Type")),',1);p.write_text(s)
p=Path('semantics/language/tests/parser_joint.rs');s=p.read_text().replace('execution.transact(id, &input.into_structured().unwrap())','execution.transact(i as u64, &input.into_structured().unwrap())');p.write_text(s)
