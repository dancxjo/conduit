# Products proof Cargo context

The Products shard passes its canonical Cargo package and feature arguments to
the committed TODO cross-Mask proof. The Rust shard definition remains the sole
authority. Both CLI dispatch routes carry a bounded, versioned private argument;
the Node runner validates a closed set of Cargo options before execution.
Standalone proof invocation retains its original two-package context.

The owner proof and ignored browser proof still run through Cargo, in order,
against the same fresh exchange. Each must execute exactly one passing selected
test. Package coverage, assertions, overflow checks, and timeouts are retained.

A complete local `cargo xtask ci pipeline unit products` replay on candidate
`92e2b1a44d0e6dfa6c8c2e1c470013a7a4876119` passed. Its initial optimized
build took 14m44s. At the proof boundary only Browser Runtime compiled (56.93s);
the following proof reused the context (0.26s). Repeated product harnesses used
the same executable paths. The preceding accepted CI run spent 14m20s rebuilding
at this boundary, after a 19m11s initial build. These are different machines and
cache states: the local replay proves reuse, not a controlled end-to-end CI
speedup. A fresh Integration run must establish the resulting CI duration.
