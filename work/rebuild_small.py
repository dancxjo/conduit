from pathlib import Path
import re
p=Path('semantics/language/parser.conduit');old=p.read_text();prefix=old[:old.index('plot language-parser-initialize')]
prefix=prefix.replace('LanguageDependencyRelation','LanguageParserRelation')
prefix='''# Four-token symbolic profile. Proposed actions and raw plotting state are not
# admitted linguistic truth. Checked plots validate states and own graph mutation.
# Root=4, unassigned=5. No beam, learned scorer, lexical analysis or speech claim.
# Empty subtype encodes absence; nonempty suffixes are admitted canonically later.
type LanguageParserSubtype = Text <= 32B
type LanguageParserRelation = {
    base: LanguageUniversalDependencyRelation
    subtype: LanguageParserSubtype
}
'''+prefix[prefix.index('type LanguageParserBasis'):]
a=prefix.index('type LanguageParserContext');b=prefix.index('\n}',a)+2
prefix=prefix[:a]+'''type LanguageParserContext = {
    request: LanguageParserRequest
    top: U64
    admitted: Boolean
    legal: Boolean
    dependent: U64
    ancestor: U64
    cycle: Boolean
}'''+prefix[b:]
# Reuse all native State laws as the source admission predicate.
a=prefix.index('type LanguageParserState');b=prefix.index('\n}',a)
laws=[l[10:] for l in prefix[a:b].splitlines() if l.startswith('    where ')]
def tree(xs,op=' && '):
 if len(xs)==1:return xs[0]
 mid=len(xs)//2;return '('+tree(xs[:mid],op)+op+tree(xs[mid:],op)+')'
scalar=['.token_count >= 1','.token_count <= 4','.unread <= 4','.depth >= 1','.depth <= 5','.committed <= 4']
valid=tree(scalar+laws);contextvalid=re.sub(r'(?<![A-Za-z0-9_])\.(?=[A-Za-z_])','.request.state.',valid)
fields=['request','top','admitted','legal','dependent','ancestor','cycle']
def ctx(**changed):return '{ '+', '.join(f'{f}: {changed.get(f,"."+f)}' for f in fields)+' }'
def plot(name,inp,out,expr):return f'plot {name} (\n    input: {inp}...| >> output: {out}...|\n) = ({expr})\n\n'
s=prefix+plot('language-parser-initialize','LanguageParserBegin','LanguageParserNumericState','{ basis: .basis, token_count: .token_count, unread: 0, depth: 1, stack: [4,4,4,4,4], heads: [5,5,5,5,4], '+', '.join(f'relation{i}: .default_relation' for i in range(4))+', committed: 0 }')
top='(.state.depth >= 1 && .state.depth <= 5 ? sequence/at(.state.stack, .state.depth - 1) : 4)'
s+=plot('language-parser-context','LanguageParserRequest','LanguageParserContext',ctx(request='.',top=top,admitted='false',legal='false',dependent=f'(variant/is(.action,"left_arc") ? {top} : (variant/is(.action,"right_arc") ? .state.unread : 5))',ancestor=f'(variant/is(.action,"left_arc") ? .state.unread : (variant/is(.action,"right_arc") ? {top} : 4))',cycle='false'))
s+=plot('language-parser-state-valid','LanguageParserNumericState','Boolean',valid)
s+=plot('language-parser-admit','LanguageParserContext','LanguageParserContext',ctx(admitted=contextvalid))
st='.request.state';base='variant/is(.request.relation.base,"root")'
shift=tree([f'{st}.unread < {st}.token_count',f'{st}.depth < 5'])
reduce=tree([f'{st}.depth > 1','.top < 4',f'sequence/at({st}.heads,.top) < 5'])
left=tree([f'{st}.unread < {st}.token_count',f'{st}.depth > 1','.top < 4',f'.top >= {st}.committed',f'sequence/at({st}.heads,.top) == 5',f'!{base}'])
root=tree(['.top == 4',base]+[f'{st}.heads.{i} != 4' for i in range(4)])
normal=tree(['.top < 4',f'!{base}'])
right=tree([f'{st}.unread < {st}.token_count',f'{st}.depth < 5',f'{st}.unread >= {st}.committed',f'sequence/at({st}.heads,{st}.unread) == 5',f'({root} || {normal})'])
actions=tree([f'(variant/is(.request.action,"{n}") && {v})' for n,v in [('shift',shift),('reduce',reduce),('left_arc',left),('right_arc',right)]],' || ')
basis=tree([f'.request.basis.{f} == {st}.basis.{f}' for f in ['text','source_revision','analysis_revision']])
legal=tree(['.admitted',basis,actions])
s+=plot('language-parser-legal','LanguageParserContext','LanguageParserContext',ctx(legal=legal))
s+=plot('language-parser-follow','LanguageParserContext','LanguageParserContext',ctx(ancestor=f'(.ancestor < 5 ? sequence/at({st}.heads,.ancestor) : 5)',cycle='(.cycle || .ancestor == .dependent)'))
is_shift='(variant/is(.request.action,"shift") || variant/is(.request.action,"right_arc"))'
fields_state=[f'basis: {st}.basis',f'token_count: {st}.token_count',f'unread: ({is_shift} ? {st}.unread + 1 : {st}.unread)',f'depth: ({is_shift} ? {st}.depth + 1 : {st}.depth - 1)',
'stack: ['+', '.join(f'({is_shift} && {st}.depth == {i} ? {st}.unread : {st}.stack.{i})' for i in range(5))+']',
'heads: ['+', '.join(f'(variant/is(.request.action,"left_arc") && .top == {i} ? {st}.unread : (variant/is(.request.action,"right_arc") && {st}.unread == {i} ? .top : {st}.heads.{i}))' for i in range(4))+',4]']
fields_state += [f'relation{i}: ((variant/is(.request.action,"left_arc") && .top == {i}) || (variant/is(.request.action,"right_arc") && {st}.unread == {i}) ? .request.relation : {st}.relation{i})' for i in range(4)]
fields_state +=[f'committed: {st}.committed']
stale=tree([f'.request.basis.{f} != {st}.basis.{f}' for f in ['text','source_revision','analysis_revision']],' || ')
s+=plot('language-parser-apply','LanguageParserContext','LanguageParserResult','{ accepted: (.legal && !.cycle), refusal: ('+stale+' ? stale_basis(unit) : (!.admitted ? invalid_state(unit) : (!.legal ? illegal_action(unit) : (.cycle ? cycle(unit) : none(unit))))), state: (.legal && !.cycle ? { '+', '.join(fields_state)+' } : .request.state) }')
s+='''plot language-parser-transition (
    request: LanguageParserRequest...| >> result: LanguageParserResult...|
) {
    request >> language-parser-context() >> language-parser-admit() >> language-parser-legal() >> language-parser-follow() >> language-parser-follow() >> language-parser-follow() >> language-parser-follow() >> language-parser-follow() >> language-parser-apply() >> result
}

'''
arc=old[old.index('# Materialize'):].replace('LanguageDependencyRelation','LanguageParserRelation');s+=arc
p.write_text(s)
p=Path('semantics/language/tests/parser_transition.rs');t=p.read_text();t=t.replace('self.ty("LanguageDependencyRelation")','self.ty("LanguageParserRelation")').replace('unit_variant(field_type(ty, "subtype"), "none")','text(field_type(ty, "subtype"), "")')
t=t.replace('steps: ["language-parser-context", "language-parser-transition"]','steps: ["language-parser-context", "language-parser-admit", "language-parser-legal", "language-parser-follow", "language-parser-follow", "language-parser-follow", "language-parser-follow", "language-parser-follow", "language-parser-apply"]')
t=t.replace('conduit_language::LanguageDependencyRelation::from_structured(\n            field(payload, "relation").clone(),\n        )\n        .unwrap(),','conduit_language::LanguageDependencyRelation::new(conduit_language::LanguageUniversalDependencyRelation::from_structured(field(field(payload,"relation"),"base").clone()).unwrap(),None).unwrap(),')
p.write_text(t)
