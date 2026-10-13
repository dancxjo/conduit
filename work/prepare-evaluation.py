p='architecture/plot/src/expression_prepared.rs'
s=open(p).read();a=s.index('fn evaluate_node<');b=s.index('\nfn leaf_kind(',a)
node=s[a:b].replace('fn evaluate_node<','pub(super) fn evaluate_node<',1)
node=node.replace('    input_kind: Option<PrimitiveInfoKind>,\n','    input_kind: Option<PrimitiveInfoKind>,\n    canonical: EvaluationInput<\'a>,\n',1)
node=node.replace('input, input_kind)', 'input, input_kind, canonical)')
node=node.replace('inspection.evaluate(input)', 'inspection.evaluate(canonical)').replace('selection.evaluate(input)','selection.evaluate(canonical)').replace('projection.evaluate(input)','projection.evaluate(canonical)')
open('architecture/plot/src/expression_prepared/evaluation.rs','w').write('//! Bounded traversal over one admitted immutable input.\nuse super::*;\n\n'+node+'\n')
s=s[:a]+s[b:];s=s.replace('mod byte_observation;','mod byte_observation;\nmod evaluation;\nuse evaluation::evaluate_node;\nmod input;\nuse input::EvaluationInput;\npub(crate) use input::PreparedCanonicalInput;')
s=s.replace('        let mut primitive_bytes = input;','        self.evaluate_input(EvaluationInput::new(input))\n    }\n\n    pub(crate) fn evaluate_canonical(&mut self, input: &PreparedCanonicalInput<\'_>) -> Result<&[u8], Refusal> {\n        self.evaluate_input(input.input())\n    }\n\n    fn evaluate_input(&mut self, mut input: EvaluationInput<\'_>) -> Result<&[u8], Refusal> {\n        let mut primitive_bytes = input.bytes();')
s=s.replace('nominal::input_payload(input, expected)', 'nominal::input_payload(input.bytes(), expected)')
s=s.replace('                let validated = conduit_core::validate_canonical_structured_value(input)\n                    .map_err(|_| Refusal::InvalidInput)?;', '                input = input.validate_structured()?;\n                let validated = input.structured()?;')
s=s.replace('evaluate_node(node, primitive_bytes, primitive_input)', 'evaluate_node(node, primitive_bytes, primitive_input, input)')
open(p,'w').write(s)
# Forward the admitted input through structured child programs.
p='architecture/plot/src/expression_prepared/structured.rs';s=open(p).read();s='use super::EvaluationInput;\n'+s if not s.startswith('//!') else s
# inner doc comments must stay first; insert after doc lines
if 'use super::EvaluationInput;' not in s:
 lines=s.splitlines(True);i=0
 while i<len(lines) and lines[i].startswith('//!'):i+=1
 lines.insert(i,'use super::EvaluationInput;\n');s=''.join(lines)
s=s.replace('input: &[u8]', 'input: EvaluationInput<\'_>').replace('.evaluator.evaluate(input)', '.evaluator.evaluate_input(input)').replace('append(output, input)', 'append(output, input.bytes())')
open(p,'w').write(s)
for name in ['inspection','sequence_selection']:
 p='architecture/plot/src/expression_prepared/'+name+'.rs';s=open(p).read();s=s.replace('use super::{','use super::{\n    EvaluationInput,',1)
 s=s.replace("input: &'a [u8]", "input: EvaluationInput<'a>").replace('input: &[u8]', "input: EvaluationInput<'_>")
 s=s.replace('.evaluate(input)', '.evaluate_input(input)')
 open(p,'w').write(s)
p='architecture/plot/src/expression_prepared/member_selection.rs';s=open(p).read().replace('use super::{','use super::{EvaluationInput,',1)
s=s.replace("input: &'a [u8]", "input: EvaluationInput<'a>")
s=s.replace('let mut value = conduit_core::validate_canonical_structured_value(input)\n            .map_err(|_| Refusal::InvalidInput)?;', 'let mut value = input.structured()?;')
open(p,'w').write(s)
p='architecture/plot/src/expression_prepared/byte_observation.rs';s=open(p).read().replace('use super::{','use super::{\n    EvaluationInput,',1)
s=s.replace('        input_kind: Option<PrimitiveInfoKind>,\n','        input_kind: Option<PrimitiveInfoKind>,\n        canonical: EvaluationInput<\'_>,\n')
s=s.replace('input, input_kind)', 'input, input_kind, canonical)')
open(p,'w').write(s)
