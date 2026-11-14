# Precise next whole-factory refusal

The numeric trace log now follows the existing shared Source execution entrance: 237 event slots and `237 * size_of::<KernelEvent>()` concrete trace bytes (4,740 on CPU). The original 15,277-byte named mandatory-Sign receipt budget is unchanged and remains a different Plan representation. The concrete trace constructor succeeds.

The complete Source76 factory then fails at `PreparedFragment::prepare_with_terminal_transductions_ref` with **`Tables(Capacity)`**.

This inner error was observed through a CPU-only temporary copy of the original neutral factory. The harness instruments only the error-erasing `neutral-kernel` and `neutral-workspace` `map_err` closures, logs the original Debug refusal, and returns the same static refusal. No checks, records, capacities, flags, control branches or provider operations change. The original repository factory and the no_std probe remain unmodified by this observation.

Original factory SHA-256: `3251c2ad4a4511e1aa34e90f46feb38ef398e4ec5ba7992ae706953f3377c4fe`.
Diagnostic copy SHA-256: `3ea2531177cef2db7369c6875897a731c442b48ed23270234eee1e83c3933604`.

The next authoritative source guard is `architecture/kernel/src/assigned/layout.rs`: `validate` rejects `layout.records.len() > 256` before using its `[false; 256]` tracking buffer. The exact emitted Source76 layout contains **721 immutable record requirements**, after the existing exclusion of configuration, Sign and terminal records. Thus this guard refuses before per-record topology checks. No record is established malformed.

A memory-preserving general option is to reuse the same 256 bytes as bitmaps over tag-local requirement ordinals: 32 bytes per tag across at most eight tags per pass, two passes over the existing 13-tag/255-record-per-tag canonical domain. Exact digest matching, duplicate multiplicity and missing/excess refusal must remain. This is a proposal, not implemented by this child.

Authoritative log: `outputs/positive-kernel-inner-refusal-20261114.log`, terminal failure of the generated CPU test after 18.94 seconds. It also preserves original profile validation success, 30,314-byte signed command under unchanged 32,548-byte input, and numeric trace readiness. Full kernel construction, execution workspace, admission, Start and target heap fit remain unproved. Compiler and cache leases are released.
