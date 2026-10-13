from pathlib import Path
p=Path('semantics/language/parser_beam.conduit');s=p.read_text()
# A fixed four-item sorting network; active candidates first, score descending,
# then exact proposal identity ascending. No scores are normalized into truth.
for a,b in [(0,1),(2,3),(0,2),(1,3),(1,2)]:
 left=f'.candidate{a}';right=f'.candidate{b}'
 wins=f'({left}.active && !{right}.active) || ({left}.active == {right}.active && ({left}.score > {right}.score || ({left}.score == {right}.score && {left}.identity <= {right}.identity)))'
 fields=[]
 for i in range(4):
  val=f'({wins} ? {left} : {right})' if i==a else (f'({wins} ? {right} : {left})' if i==b else f'.candidate{i}')
  fields.append(f'candidate{i}: {val}')
 s+=f'\nplot language-parser-rank-{a}-{b} (\n input: LanguageParserBeam...| >> output: LanguageParserBeam...|\n) = ({{ '+', '.join(fields)+' })\n'
s+='\nplot language-parser-rank (\n input: LanguageParserBeam...| >> output: LanguageParserBeam...|\n) {\n input >> '+' >> '.join(f'language-parser-rank-{a}-{b}()' for a,b in [(0,1),(2,3),(0,2),(1,3),(1,2)])+' >> output\n}\n'
fields=[]
for i in range(4):
 c=f'.beam.candidate{i}'
 fields.append(f'candidate{i}: {{ state: {c}.state, identity: {c}.identity, score: {c}.score, active: ({c}.active && .maximum_survivors > {i}) }}')
s+='\nplot language-parser-retain (\n input: LanguageParserPruneRequest...| >> output: LanguageParserBeam...|\n) = ({ '+', '.join(fields)+' })\n'
# The first active candidate is the reference; agreement queries must contain
# at least one admitted candidate and an existing token occurrence.
for i in range(4):
 c=f'.beam.candidate{i}';st=c+'.state'
 relation=f'(.dependent == 0 ? {st}.relation0 : (.dependent == 1 ? {st}.relation1 : (.dependent == 2 ? {st}.relation2 : {st}.relation3)))'
 s+=f'\nplot language-parser-edge-vote-{i} (\n input: LanguageParserAgreementQuery...| >> vote: LanguageParserEdgeVote...|\n) = ({{ basis: {st}.basis, dependent: .dependent, head: sequence/at({st}.heads,.dependent), relation: {relation}, active: ({c}.active && .dependent < {st}.unread) }})\n'
# Seed/reference selection is a separate short expression to respect the generic
# decoder's finite depth. Full candidate/state admission is a caller obligation.
s+='''
type LanguageParserVotes = {
    vote0: LanguageParserEdgeVote
    vote1: LanguageParserEdgeVote
    vote2: LanguageParserEdgeVote
    vote3: LanguageParserEdgeVote
}
type LanguageParserVoteCheck = {
    votes: LanguageParserVotes
    reference: LanguageParserEdgeVote
    agrees: Boolean
    survivors: U64
}
plot language-parser-agreement-start (
 input: LanguageParserVotes...| >> check: LanguageParserVoteCheck...|
) = ({ votes: ., reference: (.vote0.active ? .vote0 : (.vote1.active ? .vote1 : (.vote2.active ? .vote2 : .vote3))), agrees: true, survivors: 0 })
'''
for i in range(4):
 v=f'.votes.vote{i}'
 eq=f'({v}.basis.text == .reference.basis.text && ({v}.basis.source_revision == .reference.basis.source_revision && {v}.basis.analysis_revision == .reference.basis.analysis_revision))'
 same=f'({v}.dependent == .reference.dependent && ({v}.head == .reference.head && {v}.relation == .reference.relation))'
 s+=f'\nplot language-parser-agreement-check-{i} (\n input: LanguageParserVoteCheck...| >> check: LanguageParserVoteCheck...|\n) = ({{ votes: .votes, reference: .reference, agrees: (.agrees && (!{v}.active || ({eq} && {same}))), survivors: .survivors + ({v}.active ? 1 : 0) }})\n'
s+='''
plot language-parser-agreement-finish (
 input: LanguageParserVoteCheck...| >> agreement: LanguageParserAgreement...|
) = ({ reference: .reference, agrees: (.agrees && .survivors > 0 && .reference.head < 5), survivors: .survivors })
plot language-parser-agreement (
 votes: LanguageParserVotes...| >> agreement: LanguageParserAgreement...|
) {
 votes >> language-parser-agreement-start() >> language-parser-agreement-check-0() >> language-parser-agreement-check-1() >> language-parser-agreement-check-2() >> language-parser-agreement-check-3() >> language-parser-agreement-finish() >> agreement
}
'''
p.write_text(s)
