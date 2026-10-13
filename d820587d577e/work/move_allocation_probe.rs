use std::alloc::{GlobalAlloc,Layout,System};use std::sync::atomic::{AtomicBool,AtomicUsize,Ordering};
pub struct Probe;pub static ENABLED:AtomicBool=AtomicBool::new(false);pub static COUNT:AtomicUsize=AtomicUsize::new(0);
unsafe impl GlobalAlloc for Probe {
 unsafe fn alloc(&self,l:Layout)->*mut u8{if ENABLED.load(Ordering::Relaxed){COUNT.fetch_add(1,Ordering::Relaxed);}unsafe{System.alloc(l)}}
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){unsafe{System.dealloc(p,l)}}
 unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{if ENABLED.load(Ordering::Relaxed){COUNT.fetch_add(1,Ordering::Relaxed);}unsafe{System.realloc(p,l,n)}}
 unsafe fn alloc_zeroed(&self,l:Layout)->*mut u8{if ENABLED.load(Ordering::Relaxed){COUNT.fetch_add(1,Ordering::Relaxed);}unsafe{System.alloc_zeroed(l)}}
}
#[global_allocator]static ALLOCATOR:Probe=Probe;
pub fn allocations<T>(f:impl FnOnce()->T)->(T,usize){COUNT.store(0,Ordering::Relaxed);ENABLED.store(true,Ordering::Relaxed);let v=f();ENABLED.store(false,Ordering::Relaxed);(v,COUNT.load(Ordering::Relaxed))}
