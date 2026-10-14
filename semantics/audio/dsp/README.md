# Portable seeded excitation

`seeded-noise.conduit` defines `dsp/noise-step` as an ordinary checked Source
Plot. Its single generic pure-expression Back uses no clock, device, ambient
randomness, speech parser, or voice policy.

The exact `lcg16` recurrence is `(word * 25173 + 13849) mod 65536`.
Every 16-bit word, including zero, is an admitted seed. The explicit state
also carries remaining work: one to 65,536 steps. An `advanced` result contains
the next word, decremented remaining work, and a centered signed sample
`next_word - 32768` in −32768..32767. These are integer excitation samples;
no sample rate, physical amplitude, audio device, or noise-quality guarantee
is implied.

Source guards return `invalid_seed`, `exhausted`, or `invalid_profile` without
performing the update or producing a sample. Type annotations describe the
profile; the guards enforce it even for malformed caller state. The output
record uses ordinary integer fields; the arithmetic and full-period reference
proof establish its range. Reentry checks the input profile again.

Reset supplies the same seed and work count. Resume supplies the exact last
advanced word and remaining count. There is no hidden generator state or
Rust noise implementation. Callers retain state explicitly and must stop on
refusal. Chunk boundaries and fresh Host/Play preparation do not alter the
sequence. The Host's existing maximum of 64 input values per external sequence
still applies; it is independent of the component's total remaining-work bound.

Include the declarations in authored Source, check them with the current
catalogue, expand `dsp/noise-step`, and prepare the existing exact
`pure_expression_std_offer` for its compiled program and temporal contract.
Ordinary planning seals the selected generic Back. The source contains no
Host or resource binding.

The accompanying plot tests evaluate the complete 65,536-step period against
an independent wide-integer reference and verify fixed prepared-output
capacity. The std Host tests execute real Kernel HostCalls, compare complete
output bytes across reset/partition/fresh-Host resume, check exact Source
custody, and verify unchanged Kernel value-allocation capacity. These are
deterministic hosted execution receipts.
