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
fn exchange(state:&mut State,bytes:&[u8],transfer:&mut[u8])->Vec<u8>{
 let mut result=Vec::with_capacity(conduitos_application_core::MAX_BLOB);
 ACTIVE.store(true,Ordering::SeqCst);
 assert_eq!(state.execute(Request::Begin{bytes:bytes.len() as u32},transfer),Ok(Response::Ready));
 for (index,chunk) in bytes.chunks(transfer.len()).enumerate(){transfer[..chunk.len()].copy_from_slice(chunk);assert_eq!(state.execute(Request::Write{offset:(index*transfer.len())as u32,bytes:chunk.len()as u32},transfer),Ok(Response::Ready));}
 let Response::Blob{bytes:total}=state.execute(Request::Commit,transfer).unwrap()else{panic!("blob")};
 while result.len()<(total as usize){let offset=result.len() as u32;let maximum=(total-offset).min(transfer.len()as u32);let Response::Chunk{bytes,..}=state.execute(Request::Read{offset,maximum},transfer).unwrap()else{panic!("chunk")};result.extend_from_slice(&transfer[..bytes as usize]);}
 ACTIVE.store(false,Ordering::SeqCst);result
}
fn main(){
 let mapped=unsafe{mmap(memory::METADATA as *mut u8,memory::METADATA_BYTES,3,0x100022,-1,0)};assert_eq!(mapped as usize,memory::METADATA);
 let raw=std::fs::read("work/plan-diagnostic/init.json").unwrap();
 let command:Command=serde_json::from_slice(&raw).unwrap();let compressed=data::encode(&command).unwrap();
 println!("raw={} compressed={}",raw.len(),compressed.len());
 let length=u32::from_le_bytes(compressed[8..12].try_into().unwrap())as usize;let mut uncompressed=vec![0;length];let(status,used,written)=miniz_oxide::inflate::core::decompress(&mut miniz_oxide::inflate::core::DecompressorOxide::new(),&compressed[12..],&mut uncompressed,0,5);println!("decodeframe {:?} used={} expected={} written={} declared={} typed={:?}",status,used,compressed.len()-12,written,length,serde_json::from_slice::<Command>(&uncompressed).err());
 println!("arounderror {:?}",String::from_utf8_lossy(&uncompressed[421730..421850]));
 let roundtrip:Command=data::decode(&compressed).unwrap();assert_eq!(serde_json::to_vec(&roundtrip).unwrap(),raw);drop(roundtrip);
 println!("fixture raw={} compressed={} heap={} allocator=exact-guest32bitmap",raw.len(),compressed.len(),memory::HEAP_BYTES);
 let mut state=State::default();let mut transfer=vec![0;memory::TRANSFER_BYTES];
 let reply=exchange(&mut state,&compressed,&mut transfer);assert!(matches!(data::decode::<Reply>(&reply).unwrap(),Reply::Initialized));
 println!("Init peak={} retained={}",PEAK.load(Ordering::Relaxed),LIVE.load(Ordering::Relaxed));
 let Command::Init{applications}=command else{panic!("init")};
 for (slot,application)in applications.iter().enumerate(){if let Some(Initial::Patchbay{targets,..})=application {for(target,graph)in targets.iter().enumerate(){let Some(graph)=graph else{continue};let request=data::encode(&Command::Select{slot:slot as u8,target:target as u8}).unwrap();let reply=exchange(&mut state,&request,&mut transfer);assert!(matches!(data::decode::<Reply>(&reply).unwrap(),Reply::Updated{..}));let request=data::encode(&Command::Graph{slot:slot as u8}).unwrap();let reply=exchange(&mut state,&request,&mut transfer);let Reply::Graph{graph:returned}=data::decode::<Reply>(&reply).unwrap()else{panic!("graph")};assert_eq!(returned.as_ref(),Some(&graph.graph));println!("Graph target={} wire={} peak={} retained={}",target,reply.len(),PEAK.load(Ordering::Relaxed),LIVE.load(Ordering::Relaxed));}}}
 drop(state);assert_eq!(LIVE.load(Ordering::Relaxed),0);
 println!("PASS all typed Init/Graph fields preserved, guest allocator never exhausted");
}
