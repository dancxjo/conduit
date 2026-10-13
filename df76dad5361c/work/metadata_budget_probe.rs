use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator, PortableExpressionNode, PortableExpressionOperation as Op};
use conduit_core::StructuredInfoTypeShape;
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);
struct Probe;
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { let p=System.alloc(l); if !p.is_null(){let now=LIVE.fetch_add(l.size() as isize,Ordering::SeqCst)+l.size() as isize; PEAK.fetch_max(now,Ordering::SeqCst);} p }
    unsafe fn dealloc(&self, p:*mut u8,l:Layout){LIVE.fetch_sub(l.size() as isize,Ordering::SeqCst);System.dealloc(p,l)}
    unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8 { let q=System.realloc(p,l,n);if !q.is_null(){let now=LIVE.fetch_add(n as isize-l.size() as isize,Ordering::SeqCst)+n as isize-l.size() as isize;PEAK.fetch_max(now,Ordering::SeqCst);}q }
}
#[global_allocator] static A: Probe=Probe;
fn inspect(n: &PortableExpressionNode) {
 match &n.operation {
 Op::Literal(_) if !matches!(n.value_type.shape(),StructuredInfoTypeShape::Leaf(_)) => eprintln!("nonleaf_literal_type_bytes={}",n.value_type.canonical_bytes().unwrap().len()),
 Op::Projection {value,..} | Op::Unary {operand:value,..} | Op::Variant {payload:value,..} => inspect(value),
 Op::Binary {left,right,..} => {inspect(left);inspect(right);},
 Op::Conditional {condition,when_true,when_false} => {inspect(condition);inspect(when_true);inspect(when_false);},
 Op::Tuple(v) | Op::Collection(v) | Op::SemanticCall {arguments:v,..} => for c in v {inspect(c)},
 Op::Record(v) => for (_,c) in v {inspect(c)}, _ => ()
 }
}
fn main(){
 let manifest=std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
 let mut stats=std::collections::BTreeMap::<String,(usize,usize,isize,isize,u128)>::new();
 for row in manifest.lines(){let c=row.split('\t').collect::<Vec<_>>();let bytes=std::fs::read(c[2]).unwrap();
  let base=LIVE.load(Ordering::SeqCst);let t=std::time::Instant::now();let law=PortableExpressionProgram::from_canonical_bytes(&bytes).unwrap();let decode=t.elapsed().as_micros(); inspect(&law.root);let ast=LIVE.load(Ordering::SeqCst)-base; assert!(law.owned_heap_bytes() >= ast as usize);
  let before=LIVE.load(Ordering::SeqCst); PEAK.store(before,Ordering::SeqCst);let prepared=PreparedPortableExpressionEvaluator::new_with_storage_limits(&law,usize::MAX,usize::MAX,usize::MAX);let prepared_bytes=LIVE.load(Ordering::SeqCst)-before;
  let peak=PEAK.load(Ordering::SeqCst)-before;let ready=prepared.is_ok(); let bound=prepared.as_ref().map(|(p,receipt)| { assert!(receipt.preparation_requested_bytes_bound >= peak as usize, "charged {} peak {}", receipt.preparation_requested_bytes_bound, peak); p.owned_heap_bytes() }).unwrap_or(0);assert!(bound >= prepared_bytes as usize);drop(law);let retained=LIVE.load(Ordering::SeqCst)-base;assert_eq!(retained,prepared_bytes);drop(prepared);assert_eq!(LIVE.load(Ordering::SeqCst),base);
  eprintln!("type={} law={} ready={} encoded={} ast_retained={} prepared_retained={} decode_us={} storage_bound={} preparation_additional_peak={}",c[0],c[1],ready,bytes.len(),ast,retained,decode,bound,peak);
  let s=stats.entry(c[0].into()).or_default();s.0+=1;s.1+=bytes.len();s.2+=ast;s.3+=retained;s.4+=decode;
 }
 for (n,s) in stats {println!("family_type={n} laws={} encoded_bytes={} ast_retained_bytes={} prepared_retained_bytes={} decode_us={}",s.0,s.1,s.2,s.3,s.4);}
}
