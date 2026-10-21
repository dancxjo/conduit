use std::{alloc::{GlobalAlloc,Layout,System}, sync::atomic::{AtomicBool,AtomicUsize,Ordering}};
use conduitos_application_core::{data::{self,Command,Reply,Initial},state::State,wire::{Request,Response},memory};
mod allocator;
struct Routed;static ACTIVE:AtomicBool=AtomicBool::new(false);static LIVE:AtomicUsize=AtomicUsize::new(0);static PEAK:AtomicUsize=AtomicUsize::new(0);
unsafe extern "C" {fn write(fd:i32,p:*const u8,n:usize)->isize;}
fn report(label:&str,n:usize){let mut bytes=[0u8;80];let mut end=0;for b in label.bytes(){bytes[end]=b;end+=1;}let mut number=[0u8;20];let mut at=20;let mut n=n;loop{at-=1;number[at]=b'0'+(n%10)as u8;n/=10;if n==0{break;}}for b in &number[at..]{bytes[end]=*b;end+=1;}bytes[end]=b'\n';end+=1;unsafe{write(1,bytes.as_ptr(),end);}}
fn guest(p:*mut u8)->bool{(memory::HEAP_START..memory::HEAP_START+memory::HEAP_BYTES).contains(&(p as usize))}
fn rounded(l:Layout)->usize{l.size().max(1).div_ceil(32)*32}
unsafe impl GlobalAlloc for Routed{
 unsafe fn alloc(&self,l:Layout)->*mut u8 {if ACTIVE.load(Ordering::Relaxed){let p=unsafe{allocator::HEAP.alloc(l)};if !p.is_null(){let n=LIVE.fetch_add(rounded(l),Ordering::Relaxed)+rounded(l);PEAK.fetch_max(n,Ordering::Relaxed);}else{report("OOM request=",l.size());report("OOM roundedlive=",LIVE.load(Ordering::Relaxed));report("OOM peak=",PEAK.load(Ordering::Relaxed));}p}else{unsafe{System.alloc(l)}}}
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){if guest(p){LIVE.fetch_sub(rounded(l),Ordering::Relaxed);unsafe{allocator::HEAP.dealloc(p,l)}}else{unsafe{System.dealloc(p,l)}}}
}
#[global_allocator]static HEAP:Routed=Routed;
unsafe extern "C"{fn mmap(p:*mut u8,n:usize,prot:i32,flags:i32,fd:i32,offset:isize)->*mut u8;}
fn main(){
 let mapped=unsafe{mmap(memory::METADATA as *mut u8,memory::METADATA_BYTES,3,0x100022,-1,0)};assert_eq!(mapped as usize,memory::METADATA);
 let raw=std::fs::read("work/plan-diagnostic/init.json").unwrap();
 let command:Command=serde_json::from_slice(&raw).unwrap();
 ACTIVE.store(true,Ordering::SeqCst);
 let owned=command.clone();
 let Command::Init{applications}=owned else{panic!("init")};
 let models=conduitos_application_core::models::Models::new(applications).unwrap();
 ACTIVE.store(false,Ordering::SeqCst);
 println!("typed clone + Models peak={} retained={} heap={}",PEAK.load(Ordering::Relaxed),LIVE.load(Ordering::Relaxed),memory::HEAP_BYTES);
 drop(models);assert_eq!(LIVE.load(Ordering::Relaxed),0);
 println!("PASS modeled retained startup fits without a decoded wire buffer; this does not prove streaming decode or guest execution");
}
