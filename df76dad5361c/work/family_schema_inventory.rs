use conduit_core::{StructuredInfoType,StructuredInfoTypeShape as Shape};
use conduit_plot::PortableExpressionProgram;
use std::collections::BTreeSet;
fn walk(ty:&StructuredInfoType,out:&mut BTreeSet<String>){match ty.shape(){
Shape::Leaf(_)=>(),
Shape::Nominal{schema,representation}=>{out.insert(schema.as_str().into());walk(representation,out)},
Shape::Record{schema,fields}=>{out.insert(schema.as_str().into());for f in fields {walk(f.value_type(),out)}},
Shape::Variant{schema,cases}=>{out.insert(schema.as_str().into());for c in cases {walk(c.payload_type(),out)}},
Shape::Collection{element,..}|Shape::Sequence{element,..}=>walk(element,out),}}
fn main(){let bytes=std::fs::read(std::env::args().nth(1).unwrap()).unwrap();let p=PortableExpressionProgram::from_canonical_bytes(&bytes).unwrap();let mut out=BTreeSet::new();walk(&p.input_type,&mut out);for name in out {println!("{name}");}}
