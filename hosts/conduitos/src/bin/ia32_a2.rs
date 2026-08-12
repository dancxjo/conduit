#![no_std]
#![no_main]

// A2 and A3 deliberately share one implementation. The `ia32-a3` feature
// enables A3's larger arena, entry point, and ordinary-Form path.
include!("shared/ia32_a2_a3.rs");
