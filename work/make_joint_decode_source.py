from pathlib import Path
# Author a finite Source profile; this script generates source text only.
state = '{ ' + ', '.join(f'{name}: .hypothesis.hypothesis.parser.state.{name}' for name in ['basis','token_count','unread','depth','stack','heads','relation0','relation1','relation2','relation3','committed']) + ' }'
s = '''# Explicit lexical choice lifecycle and source-owned cumulative ranking.
# Raw expansions must be recursively admitted before retained beam publication.
type LanguageParserJointRuntimeHypothesis = {
    hypothesis: LanguageParserJointHypothesis
    selected: U64 in 0..=4
    where .selected >= .hypothesis.parser.state.unread
    where .selected <= .hypothesis.parser.state.token_count
    where .selected <= .hypothesis.parser.state.unread + 1
}
type LanguageParserJointRuntimeRawHypothesis = {
    hypothesis: LanguageParserRawJointHypothesis
    selected: U64
}
type LanguageParserRawJointHypothesis = {
    parser: LanguageParserRawHypothesis
    choices: collection U64 = 4
}
type LanguageParserJointBranchQuery = {
    hypothesis: LanguageParserJointRuntimeHypothesis
    lexical: LanguageParserJointLexical
    choice: U64 in 0..=3
    where .hypothesis.hypothesis.parser.state.unread < .hypothesis.hypothesis.parser.state.token_count
    where .hypothesis.hypothesis.parser.state.token_count == .lexical.token_count
    where .hypothesis.hypothesis.parser.state.basis.text == .lexical.tape.source.material.identity
    where .hypothesis.hypothesis.parser.state.basis.source_revision == .lexical.tape.source.material.revision
}
type LanguageParserJointBranchResult = {
    proposal: LanguageParserJointRuntimeRawHypothesis
    accepted: Boolean
}
'''
choice_legal = '.hypothesis.hypothesis.parser.state.unread < .lexical.token_count ? ('
choice_legal += ' : ('.join(f'.hypothesis.hypothesis.parser.state.unread == {i} ? .choice < sequence/length(.lexical.tape.tokens.{i}.candidates)' for i in range(4)) + ' : false' + ')' * 3 + ') : false'
choices = '[' + ', '.join(f'(.hypothesis.selected == {i} && .hypothesis.hypothesis.parser.state.unread == {i} ? .choice : .hypothesis.hypothesis.choices.{i})' for i in range(4)) + ']'
s += f'''plot language-parser-joint-branch (
    query: LanguageParserJointBranchQuery...| >> result: LanguageParserJointBranchResult...|
) = {{ proposal: {{ hypothesis: {{ parser: {{ state: {state}, active: .hypothesis.hypothesis.parser.active, identity: .hypothesis.hypothesis.parser.identity, score: .hypothesis.hypothesis.parser.score }}, choices: {choices} }}, selected: (.hypothesis.selected == .hypothesis.hypothesis.parser.state.unread && .hypothesis.selected < .lexical.token_count ? .hypothesis.selected + 1 : .hypothesis.selected) }}, accepted: (.hypothesis.selected == .hypothesis.hypothesis.parser.state.unread ? ({choice_legal}) : .choice == sequence/at(.hypothesis.hypothesis.choices, .hypothesis.hypothesis.parser.state.unread)) }}
'''
s += '''type LanguageParserJointExpansion = {
    prior: LanguageParserJointRuntimeHypothesis
    result: LanguageParserResult
    score: I64 in -229376..=229376
    identity: U64
    where .result.state.basis.text == .prior.hypothesis.parser.state.basis.text
    where .result.state.basis.source_revision == .prior.hypothesis.parser.state.basis.source_revision
    where .result.state.basis.analysis_revision == .prior.hypothesis.parser.state.basis.analysis_revision
}
plot language-parser-joint-expansion (
    expansion: LanguageParserJointExpansion...| >> proposal: LanguageParserJointRuntimeRawHypothesis...|
) = { hypothesis: { parser: { state: .result.state, active: .prior.hypothesis.parser.active && .result.accepted, identity: .identity, score: (.prior.hypothesis.parser.score + .score > 1000000 ? 1000000 : (.prior.hypothesis.parser.score + .score < -1000000 ? -1000000 : .prior.hypothesis.parser.score + .score)) }, choices: .prior.hypothesis.choices }, selected: .prior.selected }
'''
s += 'type LanguageParserJointRuntimeRawBeam = {\n' + '\n'.join(f'    candidate{i}: LanguageParserJointRuntimeHypothesis' for i in range(4)) + '\n}\n'
s += '''type LanguageParserJointRuntimeMerge = {
    beam: LanguageParserJointRuntimeRawBeam
    proposal: LanguageParserJointRuntimeHypothesis
}
'''
def better(a,b):
    a += '.hypothesis.parser'; b += '.hypothesis.parser'
    return f'(({a}.active && !{b}.active) || ({a}.active == {b}.active && ({a}.score > {b}.score || ({a}.score == {b}.score && {a}.identity <= {b}.identity))))'
pairs=[(0,1),(2,3),(0,2),(1,3),(1,2)]
for i,j in pairs:
    cond = better(f'.candidate{i}',f'.candidate{j}')
    fields = [f'candidate{k}: ({cond} ? .candidate{i} : .candidate{j})' if k==i else f'candidate{k}: ({cond} ? .candidate{j} : .candidate{i})' if k==j else f'candidate{k}: .candidate{k}' for k in range(4)]
    s += f'plot language-parser-joint-runtime-rank-{i}-{j} (\n    input: LanguageParserJointRuntimeRawBeam...| >> output: LanguageParserJointRuntimeRawBeam...|\n) = {{ '+ ', '.join(fields)+' }\n'
# Merge compares only after sorting the incoming beam, then sorts again.
s += '''plot language-parser-joint-runtime-sort-input (
    input: LanguageParserJointRuntimeMerge...| >> output: LanguageParserJointRuntimeMerge...|
) = { beam: .beam, proposal: .proposal }
'''
# Stage-shaped forwarding adapters ensure source owns the complete order.
for i,j in pairs:
    cond=better(f'.beam.candidate{i}', f'.beam.candidate{j}')
    fields=[f'candidate{k}: ({cond} ? .beam.candidate{i} : .beam.candidate{j})' if k==i else f'candidate{k}: ({cond} ? .beam.candidate{j} : .beam.candidate{i})' if k==j else f'candidate{k}: .beam.candidate{k}' for k in range(4)]
    s+=f'plot language-parser-joint-runtime-input-{i}-{j} (\n    input: LanguageParserJointRuntimeMerge...| >> output: LanguageParserJointRuntimeMerge...|\n) = {{ beam: {{ '+', '.join(fields)+' }, proposal: .proposal }\n'
cond=better('.proposal','.beam.candidate3')
s+='plot language-parser-joint-runtime-insert (\n    input: LanguageParserJointRuntimeMerge...| >> output: LanguageParserJointRuntimeRawBeam...|\n) = { '+ ', '.join([f'candidate{k}: .beam.candidate{k}' for k in range(3)]+[f'candidate3: ({cond} ? .proposal : .beam.candidate3)'])+' }\n'
s+='plot language-parser-joint-runtime-merge (\n    input: LanguageParserJointRuntimeMerge...| >> output: LanguageParserJointRuntimeRawBeam...|\n) {\n    input >> '+ ' >> '.join(f'language-parser-joint-runtime-input-{i}-{j}()' for i,j in pairs)+' >> language-parser-joint-runtime-insert() >> '+' >> '.join(f'language-parser-joint-runtime-rank-{i}-{j}()' for i,j in pairs)+' >> output\n}\n'
s += 'type LanguageParserJointRuntimeBeam = {\n    beam: LanguageParserJointBeam\n    selected: collection U64 = 4\n'
for i in range(4):
    prefix=f'.beam.candidate{i}.parser.state'
    s += f'    where .selected.{i} <= 4\n    where .selected.{i} >= {prefix}.unread\n    where .selected.{i} <= {prefix}.token_count\n    where .selected.{i} <= {prefix}.unread + 1\n'
s += '}\n'
Path('work/parser_joint_decode.conduit').write_text(s)
