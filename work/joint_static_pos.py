from pathlib import Path
p=Path('semantics/language/parser_joint.conduit');s=p.read_text();a=s.index('type LanguageParserJointPosContext = {');b=s.index('# Raw merge proposals',a)
r='type LanguageParserJointPosContext = {\n    query: LanguageParserJointChoiceQuery\n    pos: collection U64 = 4\n    selected: LanguageLexicalPos\n}\n'
r+='plot language-parser-joint-pos-begin (\n    query: LanguageParserJointChoiceQuery...| >> context: LanguageParserJointPosContext...|\n) = { query: ., pos: [16,16,16,16], selected: other(unit) }\n'
upos=[('adjective',0),('adposition',1),('adverb',2),('auxiliary',3),('coordinating_conjunction',4),('determiner',5),('interjection',6),('noun',7),('numeral',8),('particle',9),('pronoun',10),('proper_noun',11),('punctuation',12),('subordinating_conjunction',13),('symbol',14),('verb',15),('other',16)]
def anytag(items):
 if len(items)==1:return f'variant/is(.selected,"{items[0][0]}")'
 mid=len(items)//2;return f'({anytag(items[:mid])} || {anytag(items[mid:])})'
def code(items):
 if len(items)==1:return str(items[0][1])
 mid=len(items)//2;return f'({anytag(items[:mid])} ? {code(items[:mid])} : {code(items[mid:])})'
chain=['language-parser-joint-pos-begin()']
for i in range(4):
 choice=f'.query.lexical.tape.tokens.{i}.candidates.3.pos'
 for c in [2,1,0]:choice=f'(.query.choices.{i} == {c} ? .query.lexical.tape.tokens.{i}.candidates.{c}.pos : {choice})'
 choice=f'(.query.lexical.token_count <= {i} ? other(unit) : {choice})'
 name=f'language-parser-joint-pos-select-{i}';chain.append(name+'()')
 r+=f'plot {name} (\n    context: LanguageParserJointPosContext...| >> selected: LanguageParserJointPosContext...|\n) = {{ query: .query, pos: .pos, selected: {choice} }}\n'
 name=f'language-parser-joint-pos-code-{i}';chain.append(name+'()')
 expr=f'(.query.lexical.token_count <= {i} ? .pos.{i} : {code(upos)})'
 r+=f'plot {name} (\n    context: LanguageParserJointPosContext...| >> resolved: LanguageParserJointPosContext...|\n) = {{ query: .query, selected: .selected, pos: ['+', '.join(expr if j==i else f'.pos.{j}' for j in range(4))+'] }\n'
r+='plot language-parser-joint-pos (\n    query: LanguageParserJointChoiceQuery...| >> context: LanguageParserJointPosContext...|\n) {\n    query >> '+' >> '.join(chain)+' >> context\n}\n'
s=s[:a]+r+s[b:];p.write_text(s)
