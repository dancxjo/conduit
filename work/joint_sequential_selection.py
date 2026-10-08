from pathlib import Path
p=Path('semantics/language/parser_joint.conduit');s=p.read_text()
a=s.index('type LanguageParserJointPosContext = {');b=s.index('# Raw merge proposals',a)
ctx='LanguageParserJointPosContext'; tokenpair=f'({ctx}, LanguageLexicalToken)'; candidatepair=f'({ctx}, LanguageLexicalCandidate)'
r=f'type {ctx} = {{\n    query: LanguageParserJointChoiceQuery\n    pos: collection U64 = 4\n}}\n'
r+=f'plot language-parser-joint-pos-begin (\n    query: LanguageParserJointChoiceQuery...| >> context: {ctx}...|\n) = {{ query: ., pos: [16,16,16,16] }}\n'
upos=[('adjective',0),('adposition',1),('adverb',2),('auxiliary',3),('coordinating_conjunction',4),('determiner',5),('interjection',6),('noun',7),('numeral',8),('particle',9),('pronoun',10),('proper_noun',11),('punctuation',12),('subordinating_conjunction',13),('symbol',14),('verb',15),('other',16)]
chain=['language-parser-joint-pos-begin()']
for i in range(4):
 name=f'language-parser-joint-token-{i}';chain.append(name+'()')
 r+=f'plot {name} (\n    context: {ctx}...| >> token: {tokenpair}...|\n) = (., sequence/at(.query.lexical.tape.tokens, (.query.lexical.token_count <= {i} ? 0 : {i})))\n'
 name=f'language-parser-joint-candidate-{i}';chain.append(name+'()')
 r+=f'plot {name} (\n    token: {tokenpair}...| >> candidate: {candidatepair}...|\n) = (.0, sequence/at(.1.candidates, (.0.query.lexical.token_count <= {i} ? 0 : .0.query.choices.{i})))\n'
 for stage in range(6):
  expr=f'.0.pos.{i}'
  for tag,code in reversed(upos[stage*3:stage*3+3]):expr=f'(variant/is(.1.pos,"{tag}") ? {code} : {expr})'
  expr=f'(.0.query.lexical.token_count <= {i} ? .0.pos.{i} : {expr})'
  name=f'language-parser-joint-pos-{i}-{stage}';chain.append(name+'()')
  r+=f'plot {name} (\n    candidate: {candidatepair}...| >> resolved: {candidatepair}...|\n) = ({{ query: .0.query, pos: ['+', '.join(expr if j==i else f'.0.pos.{j}' for j in range(4))+'] }, .1)\n'
 name=f'language-parser-joint-resolved-{i}';chain.append(name+'()')
 r+=f'plot {name} (\n    candidate: {candidatepair}...| >> context: {ctx}...|\n) = .0\n'
r+=f'plot language-parser-joint-pos (\n    query: LanguageParserJointChoiceQuery...| >> context: {ctx}...|\n) {{\n    query >> '+' >> '.join(chain)+' >> context\n}\n'
s=s[:a]+r+s[b:];p.write_text(s)
