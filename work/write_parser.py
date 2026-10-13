from pathlib import Path
p=Path('semantics/language/parser.conduit')
source='''# Reviewed symbolic arc-eager profile: at most four token occurrences.
# Fixture actions are proposals, not learned parsing or a production beam.
# Indices 0..3 name tokens; 4 is artificial root; 5 means no assigned head.
# Source/analysis basis travels with both state and action to reject stale proposals.
type LanguageParserBasis = {
    text: LanguageTextId
    source_revision: LanguageTextRevisionId
    analysis_revision: LanguageAnalysisRevisionId
}
type LanguageParserBegin = {
    basis: LanguageParserBasis
    token_count: U64 in 1..=4
}
type LanguageParserState = {
    basis: LanguageParserBasis
    token_count: U64 in 1..=4
    unread: U64 in 0..=4
    depth: U64 in 1..=5
    stack: collection U64 = 5
    heads: collection U64 = 5
    relations: collection LanguageDependencyRelation = 4
    committed: U64 in 0..=4
    where .unread <= .token_count
    where .committed <= .unread
    where .stack.0 == 4
    where .heads.4 == 4
}
type LanguageParserAction =
    shift
    | reduce
    | left_arc LanguageDependencyRelation
    | right_arc LanguageDependencyRelation

type LanguageParserRequest = {
    basis: LanguageParserBasis
    state: LanguageParserState
    action: LanguageParserAction
}
type LanguageParserContext = {
    request: LanguageParserRequest
    top: U64
    legal: Boolean
}
type LanguageParserResult = {
    state: LanguageParserState
    accepted: Boolean
}

plot language-parser-initialize (
    begin: LanguageParserBegin...| >> state: LanguageParserState...|
) = ({ basis: .basis, token_count: .token_count, unread: 0, depth: 1,
       stack: [4, 4, 4, 4, 4], heads: [5, 5, 5, 5, 4],
       relations: [{ base: dep(unit), subtype: none(unit) }, { base: dep(unit), subtype: none(unit) }, { base: dep(unit), subtype: none(unit) }, { base: dep(unit), subtype: none(unit) }], committed: 0 })

plot language-parser-context (
    request: LanguageParserRequest...| >> context: LanguageParserContext...|
) = ({ request: ., top: sequence/at(.state.stack, .state.depth - 1), legal: false })

plot language-parser-legal (
    context: LanguageParserContext...| >> checked: LanguageParserContext...|
) = ({ request: .request, top: .top, legal:
    .request.basis.text == .request.state.basis.text
    && .request.basis.source_revision == .request.state.basis.source_revision
    && .request.basis.analysis_revision == .request.state.basis.analysis_revision
    && (.top == 4 || .top < .request.state.unread)
    && (.request.action is shift ?
        (.request.state.unread < .request.state.token_count && .request.state.depth < 5)
      : (.request.action is reduce ?
        (.request.state.depth > 1 && .top < 4 && sequence/at(.request.state.heads, .top) < 5)
      : (.request.action is left_arc ?
        (.request.state.unread < .request.state.token_count && .request.state.depth > 1
         && .top < 4 && .top >= .request.state.committed
         && sequence/at(.request.state.heads, .top) == 5
         && !(.request.action.left_arc.base is root))
      : (.request.state.unread < .request.state.token_count && .request.state.depth < 5
         && .request.state.unread >= .request.state.committed
         && sequence/at(.request.state.heads, .request.state.unread) == 5
         && (.top == 4 ? .request.action.right_arc.base is root : !(.request.action.right_arc.base is root))))) })

plot language-parser-apply (
    checked: LanguageParserContext...| >> result: LanguageParserResult...|
) = ({ accepted: .legal, state: (.legal ? {
'''
fields=['basis: .request.state.basis','token_count: .request.state.token_count',
'unread: ((.request.action is shift || .request.action is right_arc) ? .request.state.unread + 1 : .request.state.unread)',
'depth: ((.request.action is shift || .request.action is right_arc) ? .request.state.depth + 1 : .request.state.depth - 1)',
'stack: ['+', '.join(f'((.request.action is shift || .request.action is right_arc) && .request.state.depth == {i} ? .request.state.unread : .request.state.stack.{i})' for i in range(5))+']',
'heads: ['+', '.join(f'(.request.action is left_arc && .top == {i} ? .request.state.unread : (.request.action is right_arc && .request.state.unread == {i} ? .top : .request.state.heads.{i}))' for i in range(4))+', 4]',
'relations: ['+', '.join(f'(.request.action is left_arc && .top == {i} ? .request.action.left_arc : (.request.action is right_arc && .request.state.unread == {i} ? .request.action.right_arc : .request.state.relations.{i}))' for i in range(4))+']',
'committed: .request.state.committed']
source+=',\n'.join('    '+f for f in fields)+'\n} : .request.state) })\n\nplot language-parser-transition (\n    request: LanguageParserRequest...| >> result: LanguageParserResult...|\n) {\n    request >> language-parser-context() >> language-parser-legal() >> language-parser-apply() >> result\n}\n'
p.write_text(source)
