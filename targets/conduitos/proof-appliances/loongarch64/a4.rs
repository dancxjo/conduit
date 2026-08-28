#![no_std]
#![no_main]

// A4 extends A3 through `cfg(feature = "loongarch64-a4")`; its distinct
// wrapper prevents Cargo from treating two stage targets as the same source.
include!("shared/a3_a4.rs");
