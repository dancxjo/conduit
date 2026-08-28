#![no_std]
#![no_main]

// A3 and A4 deliberately share the ordinary-Form implementation. The
// `riscv64-a4` feature adds only the earned Observatory export.
include!("shared/a3_a4.rs");
