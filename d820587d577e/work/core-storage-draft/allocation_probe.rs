use conduit_core::*;
use std::alloc::{GlobalAlloc,Layout,System};
use std::sync::atomic::{AtomicBool,AtomicUsize,Ordering::Relaxed};
static ON:AtomicBool=AtomicBool::new(false);static REQUESTED:AtomicUsize=AtomicUsize::new(0);static LIVE:AtomicUsize=AtomicUsize::new(0);static PEAK:AtomicUsize=AtomicUsize::new(0);static CALLS:AtomicUsize=AtomicUsize::new(0);
struct Alloc;unsafe impl GlobalAlloc for Alloc{
 unsafe fn alloc(&self,l:Layout)->*mut u8{if ON.load(Relaxed){charge(l.size())}unsafe{System.alloc(l)}}
 unsafe fn alloc_zeroed(&self,l:Layout)->*mut u8{if ON.load(Relaxed){charge(l.size())}unsafe{System.alloc_zeroed(l)}}
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){if ON.load(Relaxed){LIVE.fetch_sub(l.size(),Relaxed);}unsafe{System.dealloc(p,l)}}
 unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{if ON.load(Relaxed){charge(n);LIVE.fetch_sub(l.size(),Relaxed);}unsafe{System.realloc(p,l,n)}}
}
#[global_allocator]static ALLOC:Alloc=Alloc;
fn charge(n:usize){CALLS.fetch_add(1,Relaxed);REQUESTED.fetch_add(n,Relaxed);let peak=LIVE.fetch_add(n,Relaxed)+n;PEAK.fetch_max(peak,Relaxed);}
fn measure<T>(f:impl FnOnce()->T)->(T,usize,usize,usize){for a in [&REQUESTED,&LIVE,&PEAK,&CALLS]{a.store(0,Relaxed)}ON.store(true,Relaxed);let value=f();ON.store(false,Relaxed);(value,REQUESTED.load(Relaxed),PEAK.load(Relaxed),CALLS.load(Relaxed))}
fn main(){
 let leaf=StructuredInfoType::leaf(KindId::new("value/u64")).unwrap();
 let nominal=StructuredInfoType::nominal(KindId::new("test/number"),leaf.clone()).unwrap();
 let collection=StructuredInfoType::collection(nominal.clone(),Some(4)).unwrap();
 let sequence=StructuredInfoType::sequence(collection.clone(),3).unwrap();
 let mut fields=Vec::with_capacity(19);fields.push(StructuredFieldType::new("numbers",sequence.clone()).unwrap());
 let record=StructuredInfoType::record(KindId::new("test/record"),fields).unwrap();
 let mut cases=Vec::with_capacity(11);cases.push(StructuredVariantCase::new("present",record.clone()).unwrap());
 let variant=StructuredInfoType::variant(KindId::new("test/variant"),cases).unwrap();
 for (i,ty) in [leaf,nominal,collection,sequence,record,variant].iter().enumerate(){
  let(reserve,_,_,calls)=measure(||PreparedStructuredValueValidator::storage_reservation(ty,262144).unwrap());assert_eq!(calls,0);
  let((validator,receipt),requests,peak,_)=measure(||PreparedStructuredValueValidator::new_with_storage_limits(ty,262144,reserve.preparation_requested_bytes_bound,reserve.retained_heap_bytes_bound).unwrap());
  let(actual,_,_,calls)=measure(||validator.owned_heap_bytes());assert_eq!(calls,0);assert!(requests<=receipt.preparation_requested_bytes_bound);assert!(peak<=receipt.preparation_requested_bytes_bound);assert!(actual<=receipt.retained_heap_bytes_bound);
  let(result,_,_,calls)=measure(||PreparedStructuredValueValidator::new_with_storage_limits(ty,262144,reserve.preparation_requested_bytes_bound-1,reserve.retained_heap_bytes_bound));assert_eq!(calls,0);assert!(matches!(result,Err(PreparedStructuredValidationStorageRefusal::Capacity)));
  let(result,_,_,calls)=measure(||PreparedStructuredValueValidator::new_with_storage_limits(ty,262144,reserve.preparation_requested_bytes_bound,reserve.retained_heap_bytes_bound-1));assert_eq!(calls,0);assert!(matches!(result,Err(PreparedStructuredValidationStorageRefusal::Capacity)));
  println!("shape={i} requested={requests} peak={peak} actual_retained={actual} bound={}",receipt.preparation_requested_bytes_bound);
 }
 println!("PASS six_shapes_spare_capacity_zero_allocation_reservations_and_one_under_before_allocation");
}
