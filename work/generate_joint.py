from pathlib import Path
s=['# Bounded joint alternatives retain exact lexical occurrences and source choices.', '# Scores propose rank; recursive native admission precedes agreement/publication.', 'type LanguageParserJointLexical = {', '    tape: LanguageLexicalTape', '    token_count: U64 in 1..=4', '    where sequence/length(.tape.tokens) == .token_count']
for i in range(4):
 t=f'sequence/at(.tape.tokens, {i})'
 for law in [f'{t}.identity.ordinal == {i}',f'{t}.identity.text_identity == .tape.source.material.identity',f'{t}.identity.text_revision == .tape.source.material.revision',f'sequence/length({t}.candidates) >= 1']:
  s.append(f'    where .token_count <= {i} || ({law})')
s+=['}', 'type LanguageParserJointHypothesis = {','    parser: LanguageParserHypothesis','    choices: collection U64 = 4','    where .choices.0 < 4 && .choices.1 < 4 && .choices.2 < 4 && .choices.3 < 4','}', 'type LanguageParserJointBeam = {','    lexical: LanguageParserJointLexical','    basis: LanguageParserBasis','    epoch: U64','    invocation: U64','    candidate0: LanguageParserJointHypothesis','    candidate1: LanguageParserJointHypothesis','    candidate2: LanguageParserJointHypothesis','    candidate3: LanguageParserJointHypothesis','    where .basis.text == .lexical.tape.source.material.identity','    where .basis.source_revision == .lexical.tape.source.material.revision','    where .epoch == .lexical.tape.source.sequence']
for c in range(4):
 p=f'.candidate{c}.parser'; choices=f'.candidate{c}.choices'
 for law in [f'{p}.state.token_count == .lexical.token_count',*[f'{p}.state.basis.{key} == .basis.{key}' for key in ['text','source_revision','analysis_revision']]]:
  s.append(f'    where !{p}.active || ({law})')
 for i in range(4):
  s.append(f'    where !{p}.active || (.lexical.token_count <= {i} || ({choices}.{i} < sequence/length(sequence/at(.lexical.tape.tokens, {i}).candidates)))')
s+=['}', 'type LanguageParserJointChoiceQuery = {','    lexical: LanguageParserJointLexical','    state: LanguageParserState','    choices: collection U64 = 4','    epoch: U64','    invocation: U64','    where .epoch == .lexical.tape.source.sequence','    where .state.token_count == .lexical.token_count','    where .state.basis.text == .lexical.tape.source.material.identity','    where .state.basis.source_revision == .lexical.tape.source.material.revision']
for i in range(4):
 s.append(f'    where .lexical.token_count <= {i} || (.choices.{i} < sequence/length(sequence/at(.lexical.tape.tokens, {i}).candidates))')
s+=['}', '# Raw POS projection must be admitted as PosEvidence and joined to this State.', 'type LanguageParserJointPosContext = {','    query: LanguageParserJointChoiceQuery','    pos: collection U64 = 4','}', 'plot language-parser-joint-pos-begin (','    query: LanguageParserJointChoiceQuery...| >> context: LanguageParserJointPosContext...|',') = { query: ., pos: [16,16,16,16] }']
# Each stage maps three POS names at most, bounding source AST nesting.
upos=[('adjective',0),('adposition',1),('adverb',2),('auxiliary',3),('coordinating_conjunction',4),('determiner',5),('interjection',6),('noun',7),('numeral',8),('particle',9),('pronoun',10),('proper_noun',11),('punctuation',12),('subordinating_conjunction',13),('symbol',14),('verb',15),('other',16)]
for stage in range(6):
 values=[]
 for i in range(4):
  pos=f'sequence/at(sequence/at(.query.lexical.tape.tokens, {i}).candidates, .query.choices.{i}).pos'
  expr=f'.pos.{i}'
  for name,code in reversed(upos[stage*3:stage*3+3]):
   expr=f'(variant/is({pos},"{name}") ? {code} : {expr})'
  values.append(f'(.query.lexical.token_count <= {i} ? .pos.{i} : {expr})')
 s += [f'plot language-parser-joint-pos-{stage} (','    context: LanguageParserJointPosContext...| >> resolved: LanguageParserJointPosContext...|',f') = {{ query: .query, pos: [{", ".join(values)}] }}']
s+=['plot language-parser-joint-pos (','    query: LanguageParserJointChoiceQuery...| >> context: LanguageParserJointPosContext...|',') {','    query >> language-parser-joint-pos-begin() >> '+' >> '.join(f'language-parser-joint-pos-{i}()' for i in range(6))+' >> context','}']
Path('semantics/language/parser_joint.conduit').write_text('\n'.join(s)+'\n')
p=Path('semantics/language/build.rs'); s=p.read_text(); s=s.replace('    println!("cargo:rerun-if-changed=parser_mask.conduit");','    println!("cargo:rerun-if-changed=parser_mask.conduit");\n    println!("cargo:rerun-if-changed=parser_joint.conduit");'); s=s.replace('"{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}",','"{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}",'); s=s.replace('        include_str!("parser_mask.conduit"),','        include_str!("parser_mask.conduit"),\n        include_str!("parser_joint.conduit"),'); p.write_text(s)
