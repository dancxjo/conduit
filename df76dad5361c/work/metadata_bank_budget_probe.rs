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
fn main() {
 use conduit_plot::rust_binding::{PreparedNativeInvariantAdmission, PreparedNativeInvariantStorageLimits};
 let manifest=std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
 let mut groups=std::collections::BTreeMap::<String,Vec<Vec<u8>>>::new();
 for row in manifest.lines() { let columns=row.split('\t').collect::<Vec<_>>(); groups.entry(columns[0].into()).or_default().push(std::fs::read(columns[2]).unwrap()); }
 let mut total=0;
 for (name, encoded) in groups {
  let first=PortableExpressionProgram::from_canonical_bytes(&encoded[0]).unwrap();
  let PortableExpressionProgram {input_type, output_type, root}=first;
  drop(output_type);drop(root);
  let references=encoded.iter().map(Vec::as_slice).collect::<Vec<_>>();
  let limits=PreparedNativeInvariantStorageLimits { maximum_laws: encoded.len(), maximum_input_bytes:usize::MAX, maximum_retained_bytes:usize::MAX, maximum_preparation_peak_bytes:usize::MAX };
  let before=LIVE.load(Ordering::SeqCst);PEAK.store(before,Ordering::SeqCst);
  let (bank,receipt)=PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(&input_type,&references,limits).unwrap();
  let retained=LIVE.load(Ordering::SeqCst)-before;let peak=PEAK.load(Ordering::SeqCst)-before;
  assert!(receipt.retained_heap_bytes_bound >= retained as usize);
  assert!(receipt.preparation_peak_heap_bytes_bound >= peak as usize);
  assert_eq!(bank.owned_heap_bytes(),receipt.retained_heap_bytes_bound);
  eprintln!("type={name} laws={} retained={} retained_bound={} peak={} peak_bound={}",encoded.len(),retained,receipt.retained_heap_bytes_bound,peak,receipt.preparation_peak_heap_bytes_bound);
  total+=encoded.len();drop(bank);assert_eq!(LIVE.load(Ordering::SeqCst),before);
 }
 assert_eq!(total,118);
}
