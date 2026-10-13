from pathlib import Path
p=Path('semantics/language/parser_joint.conduit');s=p.read_text();s+='\n# Raw merge proposals retain lexical choices together with parser state.\n# The containing JointBeam must be recursively admitted after each expansion.\n'
s+='type LanguageParserJointRawBeam = {\n'+''.join(f'    candidate{i}: LanguageParserJointHypothesis\n' for i in range(4))+'}\n'
s+='type LanguageParserJointMerge = {\n    beam: LanguageParserJointRawBeam\n    proposal: LanguageParserJointHypothesis\n}\n'
def before(a,b):
 return f'(({a}.parser.active && !{b}.parser.active) || ({a}.parser.active == {b}.parser.active && ({a}.parser.score > {b}.parser.score || ({a}.parser.score == {b}.parser.score && {a}.parser.identity <= {b}.parser.identity))))'
s+='plot language-parser-joint-insert (\n    merge: LanguageParserJointMerge...| >> beam: LanguageParserJointRawBeam...|\n) = { '+', '.join(f'candidate{i}: '+(f'({before(".proposal",".beam.candidate3")} ? .proposal : .beam.candidate3)' if i==3 else f'.beam.candidate{i}') for i in range(4))+' }\n'
for a,b in [(0,1),(2,3),(0,2),(1,3),(1,2)]:
 pred=before(f'.candidate{a}',f'.candidate{b}')
 s+=f'plot language-parser-joint-rank-{a}-{b} (\n    input: LanguageParserJointRawBeam...| >> output: LanguageParserJointRawBeam...|\n) = {{ '+', '.join(f'candidate{i}: '+(f'({pred} ? .candidate{a} : .candidate{b})' if i==a else f'({pred} ? .candidate{b} : .candidate{a})' if i==b else f'.candidate{i}') for i in range(4))+' }\n'
s+='plot language-parser-joint-merge (\n    merge: LanguageParserJointMerge...| >> beam: LanguageParserJointRawBeam...|\n) {\n    merge >> language-parser-joint-insert() >> '+' >> '.join(f'language-parser-joint-rank-{a}-{b}()' for a,b in [(0,1),(2,3),(0,2),(1,3),(1,2)])+' >> beam\n}\n'
p.write_text(s)
p=Path('semantics/language/src/lib.rs');p.write_text(p.read_text()+'\npub use generated::{LanguageParserJointRawBeam, LanguageParserJointMerge};\n')
p=Path('semantics/language/src/parser.rs');s=p.read_text().replace('    alloc::vec![','    alloc::vec![\n        ("LanguageParserJointRawBeam", crate::LanguageParserJointRawBeam::semantic_type().expect("checked Language Type")),\n        ("LanguageParserJointMerge", crate::LanguageParserJointMerge::semantic_type().expect("checked Language Type")),',1);p.write_text(s)
