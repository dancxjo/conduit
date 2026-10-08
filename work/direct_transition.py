from pathlib import Path
import re
p=Path('semantics/language/parser.conduit');s=p.read_text();lines=s.splitlines()
legal=next(l for l in lines if l.startswith(') = ({ request: .request') and 'legal: .legal &&' in l);legal=legal.split('legal: ',1)[1][:-3].strip()
valid=next(l for l in lines if l.startswith(') = (.token_count'));valid=valid[5:-1]
validreq=re.sub(r'(?<![A-Za-z0-9_])\.(?=[A-Za-z_])','.state.',valid)
top='(.state.depth >= 1 && .state.depth <= 5 ? sequence/at(.state.stack, .state.depth - 1) : 4)'
legal=legal.replace('.legal && ', '').replace('.request.', '.').replace('.top',top)
validreq=validreq.replace('.request.state.', '.state.')
# Four token ancestors suffice; roots/unassigned sentinels never equal unread.
ancestor=top;terms=[]
for _ in range(4):
 ancestor=f'sequence/at(.state.heads, ({ancestor} < 5 ? {ancestor} : 4))'
 terms.append(f'({ancestor} != .state.unread)')
acyclic='(!variant/is(.action, "right_arc") || ('+' && '.join(terms)+'))'
apply=next(l for l in lines if l.startswith(') = ({ accepted:'))
state=apply[apply.index('state: ((.legal && !.cycle) ? ')+len('state: ((.legal && !.cycle) ? '):]
state=state[:state.rindex(' : .request.state) })')]
state=state.replace('.request.','.').replace('.top',top)
guard=f'({validreq}) && ({legal}) && ({acyclic})'
stale='(.basis.text != .state.basis.text || .basis.source_revision != .state.basis.source_revision || .basis.analysis_revision != .state.basis.analysis_revision)'
# retain type definitions, initialization; replace context pipeline with direct source policies.
a=s.index('plot language-parser-context');b=s.index('# Materialize',a)
s=s[:a]+f'''plot language-parser-state-valid (
    state: LanguageParserNumericState...| >> valid: Boolean...|
) = ({valid})

plot language-parser-legal (
    request: LanguageParserRequest...| >> legal: Boolean...|
) = ({guard})

plot language-parser-transition (
    request: LanguageParserRequest...| >> result: LanguageParserResult...|
) = ({{ accepted: ({guard}), refusal: ({stale} ? stale_basis(unit) : (!({validreq}) ? invalid_state(unit) : (!({legal}) ? illegal_action(unit) : (!({acyclic}) ? cycle(unit) : none(unit))))), state: (({guard}) ? {state} : .state) }})

'''+s[b:]
s=s.replace('    | cycle\n','    | cycle\n    | invalid_state\n')
# unused Context native wrapper removed
start=s.index('type LanguageParserContext');end=s.index('\n}',start)+2;s=s[:start]+s[end:]
p.write_text(s)
p=Path('semantics/language/tests/parser_transition.rs');t=p.read_text();a=t.index('        eprintln!("statesEqual');b=t.index('            let expanded',a);t=t[:a]+'        let prepare = |name| {\n'+t[b:];a=t.index('            steps: [');b=t.index('            types:',a);t=t[:a]+'            steps: ["language-parser-transition"].into_iter().map(prepare).collect(),\n'+t[b:];p.write_text(t)
