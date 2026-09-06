#![no_std]
#![no_main]

// A3 extends the A2 machine-wake implementation through `cfg(feature =
// "ia32-a3")`; keeping a distinct bin source makes that staging explicit.
include!("shared/a2_a3.rs");
