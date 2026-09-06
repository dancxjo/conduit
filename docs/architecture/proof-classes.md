# Machine-readable proof classes

Conduit proof records use schema version 1 and exactly the vocabulary emitted by:

```text
cargo xtask --json proofs
```

The checked contract lives in `tools/xtask/src/proof.rs`. It distinguishes contract
compilation, deterministic unit proof, deterministic simulation, hosted
integration, live browser execution, live transport, firmware build, local
physical hardware, cross-host physical execution, and manual observation.

Proof class is not an ordering. A requirement accepts its own class unless that
requirement explicitly lists permitted substitutes. Consequently simulation
cannot accidentally satisfy browser, firmware, or physical acceptance;
firmware build cannot satisfy physical acceptance; and a physical run does not
silently transfer proof to another compatible implementation.

A versioned proof record binds the exact git commit and dirty state, command,
required tools or targets, named artifacts, result, supporting timestamp, and
the claims it is allowed to satisfy. Physical records additionally require an
exact host or board identity. Timestamp is metadata, never proof identity.

The catalog classifies representative current std, browser, Pico firmware,
Pico-local, and std-to-Pico commands without changing their behavior. Browser
execution remains the pinned Chromium, one-worker, zero-retry proof defined by
the existing Playwright configuration. Hardware-free tooling can inspect the
contract, but it cannot manufacture a physical record.
