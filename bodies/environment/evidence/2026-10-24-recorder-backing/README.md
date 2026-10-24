# Recorder sink preparation, CPU64

The existing native worker proof now observes the complete sink construction
closure and reports its tracked Rust heap peak and retained balance. The closure
creates the decoder, all generation encoders, selected file operations, kernel
workspaces and the reference table. All those owners survive preparation.

The 960-reading workset retains 976,105 bytes and peaks at 2,518,125 bytes.
The two-generation cases retain 136,058 bytes and peak at 1,678,078 bytes.
The sink is separately 616 inline bytes, the reference Vec 24 inline bytes,
and the exact worker future borrowing the sink 3,648 bytes. These are CPU64
measurements, not MCU bounds. Inline sizes exclude owned heap backing.

The measured closure borrows its pre-existing context and directory; it does
not release pre-existing heap ownership. The shared probe tracks allocation
balance, not pointer ownership. Producer preparation, catalog/Body State, OS
resources, allocator overhead and stack remain outside this measurement.
Complete recorder admission remains unproven.

The focused Root test passes both assignment representations, natural drain
after 960 readings, cancellation after two readings, and the original storage
failure with a durable prefix of one. The sample/write phase allocates no Rust
heap. The earlier full Root and checkpoint evidence is retained in the previous
capture; this new run is the focused joint test only. No hardware was flashed.
