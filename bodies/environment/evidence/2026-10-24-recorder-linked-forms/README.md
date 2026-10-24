# Linked schemas in recorder quantity composers

The recorder quantity codec now prepares its temperature, humidity and
observation composers with the existing build-generated static schemas. Core
checks each schema against the exact selected Type before retaining a borrowed
reference. The semantic validator remains unchanged; these bytes are not a
new Type, sensor-integer observation or display representation. Mutable output
buffers remain separately owned. Preparation may allocate; execution does not.

CPU64 fifteen-owner retention falls from 932,905 to 918,265 bytes (14,640 bytes
saved). Preparation peak falls to 2,463,165 bytes. Two-owner retention is 128,346
bytes and peak 1,673,246. Inline sink, reference Vec and worker future remain
616, 24 and 3,648 bytes. Scope and probe limitations match recorder-backing.
Relative to that first capture the retained saving is 57,840 bytes.

Two quantity-form tests verify exact round trips, Fahrenheit normalization and
refusal of inexact, overflowing, truncated and wrong-Type values. Twenty runtime
recovery tests pass. The joint native Root test passes both assignment forms,
960-reading natural drain/catalog/Body State recovery, two-reading cancellation
and original occupied-generation failure. Sample/write execution allocates no
Rust heap. Strict runtime library lint, formatting and memory ledger pass.

The previous capture owns the file-publication tests; those were not rerun for
this composer-only change. Complete producer/sink/checkpoint/OS/stack admission,
authenticated Line and combined MCU deployment remain unproven. No firmware
was flashed. Earlier captures remain immutable.
