# Release recorder header preparation headroom

After the complete immutable chunk header passes its existing extent check,
preparation shrinks its String reservation. The mutable encoded output reserves
the actual header length plus all 64 fixed-size hex samples and closing bytes,
rather than an unrelated full 4 KiB. Failure samples occupy the same admitted
size. Append/seal semantics, canonical bytes, identity, context validation and
commit ownership do not change. Allocation remains confined to preparation.

CPU64 fifteen-owner retention falls from 918,265 to 814,975 bytes, saving
103,290 bytes. Preparation peak is 2,359,875 bytes. Two-owner retention is
114,574 bytes and peak 1,659,474. The same 616-byte inline sink, 24-byte Vec
and 3,648-byte worker future remain. Relative to the first recorder-backing
capture, retained saving is 161,130 bytes. Scope excludes producer preparation,
catalog/Body State, OS resources, allocator overhead and stack. Allocator-specific
retention measurements are not portable MCU bounds.

Twenty runtime recovery tests and the joint Root test pass: full 960-reading
natural drain, committed file chunks, catalog and fresh-Boot Body State discovery,
two-reading cancellation and original occupied-generation failure, both native
assignment forms. The native sample/write phase allocates no Rust heap. Strict
runtime lint, formatting and the guarded memory ledger pass. Earlier form and
file-publication proof remains in the preceding captures; it was not rerun here.
Complete Host admission, authenticated Line and combined MCU deployment remain
unproven. No hardware image was changed.
