#![no_std]
#![allow(dead_code)]
extern crate alloc;
// Compile-only interfaces. They are not custody, display or boot implementations.
mod boot {
 pub struct BootRecord { pub hhdm_offset: u64 }
 pub fn framebuffer_reserved_range(_:u64,_:u64)->bool { false }
 pub fn bootloader_table_page(_:u64)->bool { false }
}
mod display {
 pub struct DisplayError;
 impl DisplayError { pub fn as_str(&self)-> &'static str { "compile-only" } }
 pub struct DisplayFormat;
 impl DisplayFormat { pub fn byte_len(&self)->Result<usize,DisplayError> { Err(DisplayError) } }
 pub struct RawDisplay;
 impl RawDisplay { pub fn memory_range(&self)->(u64,usize) { (0,0) } }
 pub trait PixelTarget {
  fn format(&self)->DisplayFormat;
  fn write_pixel(&mut self,x:u32,y:u32,pixel:u32)->Result<(),DisplayError>;
  fn write_span(&mut self,x:u32,y:u32,pixels:&[u32])->Result<(),DisplayError>;
 }
 impl PixelTarget for RawDisplay {
  fn format(&self)->DisplayFormat { DisplayFormat }
  fn write_pixel(&mut self,_:u32,_:u32,_:u32)->Result<(),DisplayError> { Err(DisplayError) }
  fn write_span(&mut self,_:u32,_:u32,_:&[u32])->Result<(),DisplayError> { Err(DisplayError) }
 }
}
mod machine {
 fn read_csr<const CSR:u32>()->usize {
  let value:usize;
  unsafe { core::arch::asm!("csrrd {value}, {csr}",value=out(reg)value,csr=const CSR,options(nostack)); }
  value
 }
 fn present(_: &[u8]) {}
 #[path = "/home/dancxjo/Documents/Codex/2026-10-23/goal-please-finish-and-close-https/work/conduit-loongarch-prep/targets/conduitos/src/arch/loongarch64/framebuffer/mod.rs"]
 mod framebuffer;
}
