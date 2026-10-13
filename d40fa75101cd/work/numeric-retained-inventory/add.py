from pathlib import Path
base=Path('semantics/ai/src')
def add(file,impl,expression,name='local_accounted_heap_bytes'):
 p=base/file;s=p.read_text();s+=f'\n{impl} {{\n /// Retained requested local payload capacity. Shared tensor/model Arc owners,\n /// Box root, Arc headers, allocator bookkeeping and stack are separate charges.\n pub fn {name}(&self)->usize {{ {expression} }}\n}}\n';p.write_text(s)
for file,ty,n in [('fixed_numeric_codec.rs','FixedF32VectorCodec','WIDTH'),('fixed_numeric_i16_codec.rs','FixedI16VectorCodec','N'),('fixed_numeric_index_codec.rs','FixedU16IndexCodec','WIDTH')]:add(file,f'impl<const {n}:usize> {ty}<{n}>','self.template.capacity().saturating_add(self.output.capacity())')
add('fixed_numeric_binding.rs','impl FixedTensorPortBinding','self.encoded.capacity()')
add('fixed_numeric_finite_envelope.rs','impl FiniteEnvelope','self.template.capacity().saturating_add(self.offsets.capacity().saturating_mul(core::mem::size_of::<usize>()))')
rows=[
('fixed_numeric_back.rs',"impl<'a,const INPUT:usize,const OUTPUT:usize> FixedAffineBack<'a,INPUT,OUTPUT>",'input output weights bias'),
('fixed_numeric_flow.rs','impl<const INPUT:usize,const OUTPUT:usize> FixedAffineFlowBack<INPUT,OUTPUT>','input output weights bias'),
('fixed_numeric_linear_back.rs',"impl<'a,const INPUT:usize,const OUTPUT:usize> FixedLinearBack<'a,INPUT,OUTPUT>",'input output binding'),
('fixed_numeric_linear_flow.rs','impl<const INPUT:usize,const OUTPUT:usize> FixedLinearFlowBack<INPUT,OUTPUT>','input output weights'),
('fixed_numeric_embedding_flow.rs','impl<const ROWS:usize,const WIDTH:usize> FixedEmbeddingFlowBack<ROWS,WIDTH>','output weights'),
('fixed_numeric_float_integer.rs','impl FixedFloatIntegerBack','input output'),
('fixed_numeric_scan_back.rs','impl FixedOnePoleBack','input scalar output'),
('fixed_numeric_window_back.rs','impl FixedWindowBack','value history result'),
('fixed_numeric_operations_back.rs','impl<const WIDTH:usize> FixedTanhBack<WIDTH>','input output'),
('fixed_numeric_operations_back.rs','impl<const LEFT:usize,const RIGHT:usize,const OUTPUT:usize> FixedConcatenateBack<LEFT,RIGHT,OUTPUT>','left right output'),
('fixed_numeric_operations_back.rs',"impl<'a,const ROWS:usize,const WIDTH:usize> FixedEmbeddingBack<'a,ROWS,WIDTH>",'binding output'),
]
for file,impl,fields in rows:
 e='0usize'+''.join('.saturating_add(self.'+f+'.local_accounted_heap_bytes())'for f in fields.split())
 if 'FixedAffineBack<' in impl:e+='.saturating_add(self.access.as_ref().map_or(0,|access|access.iter().fold(0usize,|total,a| total.saturating_add(a.handle.owned_heap_bytes()).saturating_add(a.authority_grant.owned_heap_bytes()))))'
 if 'FixedLinearBack<' in impl or 'FixedEmbeddingBack<' in impl:e+='.saturating_add(self.access.handle.owned_heap_bytes()).saturating_add(self.access.authority_grant.owned_heap_bytes())'
 add(file,impl,e)
for file,impl,fields,opts in [
 ('fixed_numeric_dsp_back.rs','impl<const IN:usize,const OUT:usize> FixedDspBack<IN,OUT>','input output','right'),
 ('fixed_numeric_index_back.rs','impl<const INPUT:usize,const OUTPUT:usize> FixedIndexBack<INPUT,OUTPUT>','input output','start indices'),
 ('fixed_numeric_signal_back.rs','impl<const WIDTH:usize> FixedElementwiseBack<WIDTH>','input output','right scalar')]:
 e='0usize'+''.join('.saturating_add(self.'+f+'.local_accounted_heap_bytes())'for f in fields.split())+''.join('.saturating_add(self.'+f+'.as_ref().map_or(0,|v|v.local_accounted_heap_bytes()))'for f in opts.split());add(file,impl,e)
for file,ty in [('fixed_numeric_pair_back.rs','FixedValuePairBack'),('fixed_numeric_pair_flow.rs','FixedFlowPairBack')]:add(file,'impl '+ty,'self.encoder.owned_heap_bytes().saturating_add(self.left.local_accounted_heap_bytes()).saturating_add(self.right.local_accounted_heap_bytes())')
add('closing_structured_pair.rs','impl ClosingStructuredPairBack','self.encoder.owned_heap_bytes()')
add('fixed_numeric_guard.rs','impl FixedGuardBack','self.validator.owned_heap_bytes().saturating_add(self.output.capacity())')
add('nominal_weakening.rs','impl NominalWeakeningBack','self.input.owned_heap_bytes().saturating_add(self.output_validator.owned_heap_bytes()).saturating_add(self.input_prefix.capacity()).saturating_add(self.output_prefix.capacity()).saturating_add(self.output.capacity())')
add('fixed_numeric_u16_profile.rs','impl U16ProfileBack','self.output.capacity().saturating_add(self.validator.owned_heap_bytes()).saturating_add(self.contracts.capacity().saturating_mul(core::mem::size_of::<CheckedValueContract>())).saturating_add(self.contracts.iter().fold(0usize,|total,v|total.saturating_add(v.owned_heap_bytes()))).saturating_add(self.invariants.capacity().saturating_mul(core::mem::size_of::<PreparedPortableExpressionEvaluator>())).saturating_add(self.invariants.iter().fold(0usize,|total,v|total.saturating_add(v.owned_heap_bytes())))')
add('fixed_numeric_compact_back.rs','impl<const INPUT:usize,const OUTPUT:usize> FixedCompactBack<INPUT,OUTPUT>','self.input.local_accounted_heap_bytes().saturating_add(self.output.local_accounted_heap_bytes()).saturating_add(self.resources.capacity().saturating_mul(core::mem::size_of::<(PortId,FixedTensorPortBinding)>())).saturating_add(self.resources.iter().fold(0usize,|total,(_,v)|total.saturating_add(v.local_accounted_heap_bytes())))')
