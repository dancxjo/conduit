# Native speech footprint probe

`cargo xtask prove journey native-speech --microcontroller` links the renderer
for `thumbv6m-none-eabi` with code in FLASH and fixed output storage in RAM.
The volatile input prevents the linker from replacing the renderer with one
constant phrase. There is no allocator, vector table, clock, audio driver, or
production firmware boot. Stack usage and execution time require separate proof.
This fixture measures the synthesis Back, not a complete Conduit body.
