from pathlib import Path
p=Path('semantics/language/parser_beam.conduit');s=p.read_text()
def tree(xs):
 if len(xs)==1:return xs[0]
 n=len(xs)//2;return '('+tree(xs[:n])+' && '+tree(xs[n:])+')'
valid=tree(['.frontier.stable <= 4','.frontier.committed <= .frontier.stable'])
basis=tree([f'.frontier.basis.{f} == .agreement.reference.basis.{f}' for f in ['text','source_revision','analysis_revision']])
agreed=tree(['.agreement.agrees','.agreement.survivors >= 1','.agreement.survivors <= 4','.agreement.reference.head < 5'])
next_=tree(['.agreement.reference.dependent == .frontier.stable','.frontier.stable < 4'])
guard=tree(['!.cancelled','!.pressure',valid,basis,agreed,next_])
s+=f'\nplot language-parser-advance-stable (\n input: LanguageParserAdvanceRequest...| >> result: LanguageParserFrontierResult...|\n) = ({{ accepted: {guard}, refusal: (.cancelled ? cancelled(unit) : (.pressure ? pressure(unit) : (!{valid} ? invalid_frontier(unit) : (!{basis} ? wrong_basis(unit) : (!{agreed} ? not_agreed(unit) : (!{next_} ? not_next(unit) : none(unit))))))), frontier: ({guard} ? {{ basis: .frontier.basis, stable: .frontier.stable + 1, committed: .frontier.committed }} : .frontier) }})\n'
through=tree(['.through >= .frontier.committed','.through <= .frontier.stable'])
guard=tree(['!.cancelled','!.pressure',valid,through])
s+=f'\nplot language-parser-commit-frontier (\n input: LanguageParserCommitRequest...| >> result: LanguageParserFrontierResult...|\n) = ({{ accepted: {guard}, refusal: (.cancelled ? cancelled(unit) : (.pressure ? pressure(unit) : (!{valid} ? invalid_frontier(unit) : (!{through} ? beyond_stable(unit) : none(unit))))), frontier: ({guard} ? {{ basis: .frontier.basis, stable: .frontier.stable, committed: .through }} : .frontier) }})\n'
p.write_text(s)
