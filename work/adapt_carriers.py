from pathlib import Path
p=Path('semantics/language/parser.conduit');s=p.read_text();start=s.index('type LanguageParserState = {');end=s.index('\n}',start)+2;native=s[start:end];lawlines=[l[10:] for l in native.splitlines() if l.startswith('    where ')];numeric='\n'.join(l for l in native.replace('LanguageParserState','LanguageParserNumericState').splitlines() if not l.startswith('    where '))
s=s[:start]+native+'\n\n# Private plotting carrier; the checked admission predicate below owns its laws.\n'+numeric+s[end:]
# Rewrite use sites only, preserve native carrier definition.
loc=s.index('type LanguageParserAction =');s=s[:loc]+s[loc:].replace('LanguageParserState','LanguageParserNumericState')
s=s.replace('    where .accepted ? .refusal is none : !(.refusal is none)\n','').replace('    where (.accepted ? .refusal is none : !(.refusal is none))\n','')
s=s.replace('    where (.dependent < .state.token_count)\n','')
# initialization occurs after rewrite boundary; checked request uses numeric state.
valid=' && '.join(lawlines)
insert='''plot language-parser-state-valid (
    state: LanguageParserNumericState...| >> valid: Boolean...|
) = ('''+valid+''')

plot language-parser-admit (
    context: LanguageParserContext...| >> admitted: LanguageParserContext...|
) = ({ request: .request, top: .top, dependent: .dependent, ancestor: .ancestor, cycle: .cycle, legal: '''+valid.replace('.', '.request.state.')+''' })

'''
# Prefix only field starts, not inner members (the naive replacement above would damage).
import re
insert=insert[:insert.index('plot language-parser-admit')]+'''plot language-parser-admit (
    context: LanguageParserContext...| >> admitted: LanguageParserContext...|
) = ({ request: .request, top: .top, dependent: .dependent, ancestor: .ancestor, cycle: .cycle, legal: '''+re.sub(r'(?<![A-Za-z0-9_])\.(?=[A-Za-z_])','.request.state.',valid)+''' })

'''
s=s.replace('plot language-parser-legal (',insert+'plot language-parser-legal (')
s=s.replace('cycle: .cycle, legal: .request.basis.text', 'cycle: .cycle, legal: .legal && .request.basis.text')
s=s.replace('language-parser-context() >> language-parser-legal()', 'language-parser-context() >> language-parser-admit() >> language-parser-legal()')
s=s.replace('type LanguageParserArcResult =','''type LanguageParserArcProposal = {
    dependent: LanguageAnalysisTokenRef
    governor: LanguageDependencyHead
    relation: LanguageDependencyRelation
}
type LanguageParserArcResult =''').replace('| assigned LanguageDependencyArc','| assigned LanguageParserArcProposal')
p.write_text(s)
