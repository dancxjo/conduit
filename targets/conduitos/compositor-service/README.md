# Independent compositor guest

Prerequisite image, not production compositor integration or isolation proof.
The guest depends only on the finite `compositor-core`. Root must admit the exact
`ServiceMemoryLayout::compositor()` mappings, fresh boot-local instance, request
sequence, operation/work limits, and output grant before entry. Instance numbers
are correlation, never semantic authority or possession.

Build independently (the root build does not yet stage this image):

```sh
RUSTFLAGS='-C relocation-model=static -C code-model=kernel -C panic=abort' cargo +stable build --locked --release --manifest-path targets/conduitos/compositor-service/Cargo.toml --target x86_64-unknown-none
```

The linker admits only code RX and constants RO in the 2 MiB image window. It
rejects mutable image sections and TLS. Statically resolved GOT entries are
placed in the read-only constants segment; there is no dynamic linker. Private mutable memory comes
from separate zeroed RW+NX grants; there is no allocator or framebuffer pointer.

`memory.rs` creates the two disjoint 8 MiB arena borrows exactly once and places
the actual `Compositor<'static>` inside `MaybeUninit` at the fixed private metadata
address. This is not serialized Rust state. Its lifetime is the installed image's
exclusive private grants. Entry borrows that object only for bounded dispatch;
no reference or suspended Rust stack frame survives the terminal gate. Retained
arena roles swap on upload commit and therefore must never be reconstructed on
re-entry. Root must not read or mutate private metadata/arenas while installed.
Faults, budget exhaustion, cancellation, or revocation require quarantine and
clearing before reinstall; none may resume a half-mutated Rust operation.

The 4 KiB exchange carries codec metadata only. Bulk upload and frame spans use
the distinct 64 KiB output window under a direction-specific caller grant. Root
must validate every returned sequence, coordinate, length, completion and scope
before scanout, and only send scanout acknowledgement after actual presentation.
The guest owns retained rasters, damage, composition and routing suggestions;
it cannot acknowledge a semantic Show or acquire input/display authority.

`validate-image.rs` is a small host executable using the real Root ELF/service
admission parser. Compile it with `rustc +stable --edition=2024` and pass the
independent ELF path. It checks fixed grant addresses against the actual profile.
`retained-check.rs` is a Linux host executable linked against the host-built
compositor-core rlib; it reserves the same grant addresses with
`MAP_FIXED_NOREPLACE`, checks immutable instance and replay rejection, and checks
pixels after two committed arena swaps across fresh entry calls. These are host
checks, not evidence that graphical product execution entered CPL3.
