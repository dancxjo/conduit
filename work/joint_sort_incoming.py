from pathlib import Path
p=Path('semantics/language/parser_joint.conduit');s=p.read_text()
def before(a,b):
 return f'(({a}.parser.active && !{b}.parser.active) || ({a}.parser.active == {b}.parser.active && ({a}.parser.score > {b}.parser.score || ({a}.parser.score == {b}.parser.score && {a}.parser.identity <= {b}.parser.identity))))'
extra=''
for a,b in [(0,1),(2,3),(0,2),(1,3),(1,2)]:
 pred=before(f'.beam.candidate{a}',f'.beam.candidate{b}')
 extra+=f'plot language-parser-joint-incoming-{a}-{b} (\n    input: LanguageParserJointMerge...| >> output: LanguageParserJointMerge...|\n) = {{ beam: {{ '+', '.join(f'candidate{i}: '+(f'({pred} ? .beam.candidate{a} : .beam.candidate{b})' if i==a else f'({pred} ? .beam.candidate{b} : .beam.candidate{a})' if i==b else f'.beam.candidate{i}') for i in range(4))+' }, proposal: .proposal }\n'
# incoming carrier remains raw: admission occurs on the containing JointBeam.
s=s.replace('plot language-parser-joint-merge (',extra+'plot language-parser-joint-merge (')
s=s.replace('    merge >> language-parser-joint-insert()', '    merge >> '+' >> '.join(f'language-parser-joint-incoming-{a}-{b}()' for a,b in [(0,1),(2,3),(0,2),(1,3),(1,2)])+' >> language-parser-joint-insert()')
# Merge itself must be raw because sorting constructs it; its candidates stay
# admitted value references, and containing JointBeam always re-admits outputs.
p.write_text(s)
