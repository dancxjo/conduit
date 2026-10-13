from pathlib import Path
p=Path('semantics/language/parser.conduit')
s=p.read_text();start=s.index('plot language-parser-legal-shift (');end=s.index('plot language-parser-legal (',start)
s=s[:start]+s[end:]
s=s.replace('language-parser-legal-shift() >> language-parser-legal-reduce() >> language-parser-legal-left() >> language-parser-legal-right()', 'language-parser-legal-facts() >> language-parser-fact-shift() >> language-parser-fact-reduce() >> language-parser-fact-left() >> language-parser-fact-right() >> language-parser-fact-right-root() >> language-parser-fact-right-nonroot() >> language-parser-fact-select()')
p.write_text(s)
fields=['context','shift','reduce','left','right','right_root','right_nonroot','left_ancestor','right_ancestor','left_cycle','right_cycle']
def plot(name, body, inp='LanguageParserLegalFacts', out='LanguageParserLegalFacts'):
    return f'plot {name} (\n    input: {inp}...| >> output: {out}...|\n) = {{ '+', '.join(f'{k}: {v}' for k,v in body.items())+' }\n\n'
def update(name, changes): return plot(name,{k:changes.get(k,'.'+k) for k in fields})
f='''# Shared source-owned legality facts. The transition and scoring mask use the
# same predicates; numerical ranking receives only this source's admissible mask.
# Raw mask outputs remain proposals; final recursive native state admission is
# mandatory before beam agreement, stability, commitment, or arc publication.
type LanguageParserLegalFacts = {
    context: LanguageParserContext
    shift: Boolean
    reduce: Boolean
    left: Boolean
    right: Boolean
    right_root: Boolean
    right_nonroot: Boolean
    left_ancestor: U64
    right_ancestor: U64
    left_cycle: Boolean
    right_cycle: Boolean
}
type LanguageParserMaskQuery = {
    basis: LanguageParserBasis
    state: LanguageParserNumericState
}
type LanguageParserLegalMask = {
    basis: LanguageParserBasis
    state: LanguageParserNumericState
    allowed: collection Boolean = 76
}

'''
f+=plot('language-parser-legal-facts',{'context':'.','shift':'false','reduce':'false','left':'false','right':'false','right_root':'false','right_nonroot':'false','left_ancestor':'.request.state.unread','right_ancestor':'.top','left_cycle':'false','right_cycle':'false'},inp='LanguageParserContext')
f+=update('language-parser-fact-shift',{'shift':'.context.admitted && (.context.request.state.unread < .context.request.state.token_count && .context.request.state.depth < 5)'})
f+=update('language-parser-fact-reduce',{'reduce':'.context.admitted && (.context.request.state.depth > 1 && (.context.top < 4 && sequence/at(.context.request.state.heads, .context.top) < 5))'})
f+=update('language-parser-fact-left',{'left':'.context.admitted && ((.context.request.state.unread < .context.request.state.token_count && .context.request.state.depth > 1) && (.context.top < 4 && (.context.top >= .context.request.state.committed && sequence/at(.context.request.state.heads, .context.top) == 5)))'})
f+=update('language-parser-fact-right',{'right':'.context.admitted && ((.context.request.state.unread < .context.request.state.token_count && .context.request.state.depth < 5) && (.context.request.state.unread >= .context.request.state.committed && sequence/at(.context.request.state.heads, .context.request.state.unread) == 5))'})
f+=update('language-parser-fact-right-root',{'right_root':'.right && ((.context.top == 4 && .context.request.state.heads.0 != 4) && (.context.request.state.heads.1 != 4 && (.context.request.state.heads.2 != 4 && .context.request.state.heads.3 != 4)))'})
f+=update('language-parser-fact-right-nonroot',{'right_nonroot':'.right && .context.top < 4'})
f+=plot('language-parser-fact-select',{'request':'.context.request','top':'.context.top','admitted':'.context.admitted','legal':'.context.legal || ((variant/is(.context.request.action,"shift") && .shift) || ((variant/is(.context.request.action,"reduce") && .reduce) || ((variant/is(.context.request.action,"left_arc") && (.left && !variant/is(.context.request.relation.base,"root"))) || (variant/is(.context.request.action,"right_arc") && (variant/is(.context.request.relation.base,"root") ? .right_root : .right_nonroot)))))','dependent':'.context.dependent','ancestor':'.context.ancestor','cycle':'.context.cycle'},out='LanguageParserContext')
f+=plot('language-parser-mask-request',{'basis':'.basis','state':'.state','action':'shift(unit)','relation':'.state.relation0'},inp='LanguageParserMaskQuery',out='LanguageParserRequest')
basis='.context.request.basis.text == .context.request.state.basis.text && (.context.request.basis.source_revision == .context.request.state.basis.source_revision && .context.request.basis.analysis_revision == .context.request.state.basis.analysis_revision)'
f+=update('language-parser-mask-basis',{k:f'.{k} && ({basis})' for k in ['shift','reduce','left','right_root','right_nonroot']})
f+=update('language-parser-mask-follow',{'left_ancestor':'(.left_ancestor < 5 ? sequence/at(.context.request.state.heads, .left_ancestor) : 5)','right_ancestor':'(.right_ancestor < 5 ? sequence/at(.context.request.state.heads, .right_ancestor) : 5)','left_cycle':'.left_cycle || .left_ancestor == .context.top','right_cycle':'.right_cycle || .right_ancestor == .context.request.state.unread'})
mask=['.shift','.reduce']+[('false' if i==34 else '(.left && !.left_cycle)') for i in range(37)]+[f'(.right_{"root" if i==34 else "nonroot"} && !.right_cycle)' for i in range(37)]
f+=plot('language-parser-mask-output',{'basis':'.context.request.basis','state':'.context.request.state','allowed':'['+', '.join(mask)+']'},out='LanguageParserLegalMask')
admission=' >> '.join(f'language-parser-admit-{i}()' for i in range(10))
f+='''plot language-parser-legal-mask (
    query: LanguageParserMaskQuery...| >> mask: LanguageParserLegalMask...|
) {
    query >> language-parser-mask-request() >> language-parser-context() >> '''+admission+''' >> language-parser-legal-facts() >> language-parser-fact-shift() >> language-parser-fact-reduce() >> language-parser-fact-left() >> language-parser-fact-right() >> language-parser-fact-right-root() >> language-parser-fact-right-nonroot() >> language-parser-mask-basis() >> language-parser-mask-follow() >> language-parser-mask-follow() >> language-parser-mask-follow() >> language-parser-mask-follow() >> language-parser-mask-follow() >> language-parser-mask-output() >> mask
}
'''
Path('semantics/language/parser_mask.conduit').write_text(f)
