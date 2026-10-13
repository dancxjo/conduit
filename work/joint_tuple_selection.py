from pathlib import Path
p=Path('semantics/language/parser_joint.conduit');s=p.read_text()
a=s.index('type LanguageParserJointTokenContext = {');b=s.index('type LanguageParserJointPosContext = {');s=s[:a]+s[b:]
a=s.index('plot language-parser-joint-tokens (');b=s.index('plot language-parser-joint-pos-0 (')
tokens='('+', '.join(['LanguageParserJointChoiceQuery']+['LanguageLexicalToken']*4)+')'
candidates='('+', '.join(['LanguageParserJointChoiceQuery']+['LanguageLexicalCandidate']*4)+')'
stages='plot language-parser-joint-tokens (\n    query: LanguageParserJointChoiceQuery...| >> context: '+tokens+'...|\n) = (., '+', '.join(f'sequence/at(.lexical.tape.tokens, (.lexical.token_count <= {i} ? 0 : {i}))' for i in range(4))+')\n'
stages+='plot language-parser-joint-candidates (\n    tokens: '+tokens+'...| >> context: '+candidates+'...|\n) = (.0, '+', '.join(f'sequence/at(.{i+1}.candidates, (.0.lexical.token_count <= {i} ? 0 : .0.choices.{i}))' for i in range(4))+')\n'
stages+='plot language-parser-joint-pos-begin (\n    candidates: '+candidates+'...| >> context: LanguageParserJointPosContext...|\n) = { query: .0, pos: [16,16,16,16], '+', '.join(f'candidate{i}: .{i+1}' for i in range(4))+' }\n'
s=s[:a]+stages+s[b:];p.write_text(s)
p=Path('semantics/language/src/lib.rs');s=p.read_text();s=s.replace('pub use generated::{LanguageParserJointCandidateContext, LanguageParserJointTokenContext};\n','');p.write_text(s)
p=Path('semantics/language/src/parser.rs');s=p.read_text()
for name in ['LanguageParserJointTokenContext','LanguageParserJointCandidateContext']:
 marker='            "'+name+'",';i=s.find(marker)
 if i>=0:
  start=s.rfind('        (',0,i);end=s.index('        ),',i)+len('        ),\n');s=s[:start]+s[end:]
p.write_text(s)
