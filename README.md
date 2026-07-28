# Conduit

Conduit is a portable execution model for wiring typed, capability-aware nodes,
from Unix processes to `no_std` devices.

The runtime ontology is deliberately small:

- a **node** performs a semantic computation;
- a typed, directional **port** forms its boundary;
- a bounded **cord** connects an output port to an input port.

Editable arrangements live in `.panel` files. The `conduct` command checks,
explains, and runs them:

```sh
cargo run -p conduct -- examples/hello.panel
cargo run -p conduct -- --check examples/hello.panel
cargo run -p conduct -- --explain examples/hello.panel
cat examples/hello.panel | cargo run -p conduct -- -
```

`--run` is the default. A `.panel` is source, not bytecode, ELF, or a universal
package. Compilation means validation, resolution, and lowering to an exact
execution plan; packaging and code generation are separate operations.

## Status

This repository contains the first executable Plan C foundation:

- `conduit-core`: allocator-free contracts, canonical semantic hashes, opaque
  type references, complete port/config schemas, bounded flow-policy state
  machines, directional compatibility, and plan validation;
- `conduit-panel`: the versioned `.panel` grammar, lossless CST/source AST,
  explicit module resolver, and bounded group/pool source model;
- `conduit-runtime`: a hosted registry, typed-config resolver, explainer, and
  one-shot executor, including domain type-provider discovery;
- `conduct`: the Unix command-line interface.

The initial runtime includes intentionally small proof handlers for literal
UTF-8 text, stdin, uppercase transformation, stdout, and stderr. They establish
the complete parse → check/explain → resolve → run path without pretending to
be the final standard library.

Resolved cords carry exact item/per-value/aggregate byte limits, watermarks,
and pressure policy. The allocator-free reference queue consumes fixed caller
storage and emits every pressure, loss, replacement, and wake transition.
Lifecycle is an allocator-free semantic state machine with bounded
hierarchical cancellation, deterministic terminal precedence, explicit
drain/abort queue disposition, and substitutable composite derivation.
Reusable `composite` definitions contain ordinary nodes and cords, expose only
explicit typed ports, bind configuration separately, and explain both logical
and deterministically flattened primitive views.
Authority resolution keeps fresh host capability, declared effects, scoped
grants, and exact plan bindings separate; protected diagnostics and evidence
carry redacted metadata instead of value bytes.
Exact execution plans pin implementation/artifact/host choices, finite queues,
resource and pool maxima, required authority, and logical-to-expanded
provenance under one canonical identity validated before start.
Immutable execution events link every observation to a run and exact plan,
separate append order from recorder clocks and causality, and round-trip
structurally redacted evidence through a hosted NDJSON stream.
`.panel` version 1 now supports reusable parameterized definitions, unresolved
`using` constraints, deterministic aliased imports with optional content pins,
explicit roots, compile-time keyed/indexed port groups, and finite instance
pools. Parsing and module loading remain separate from runtime lowering;
comments and formatting round-trip without entering semantic source identity.

## Design boundaries

- Semantic node contracts are distinct from Rust, WASM, Python, firmware,
  native, or remote implementations.
- Hosts advertise capabilities and resources; implementations declare
  requirements. Conduit does not provision hosts.
- Composite arrangements become reusable by exporting typed ports and remain
  nodes at their boundary. There is no separate runtime `Panel` kind.
- Live flow is bounded. Pressure, delivery, cancellation, and terminal behavior
  are semantic rather than incidental library behavior.
- Presentation belongs to Patchbay and cannot change execution semantics.
- Tongues and Netherwick supply domain contracts; Conduit does not contain
  speech or robot-specific types.

See [`spec/`](spec/) for the evidence baseline, bootstrap meta-model, and
conformance roadmap.

## Development

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 conformance/c1/verify_canonical_v1.py
cargo check -p conduit-core --no-default-features \
  --target thumbv6m-none-eabi
```

The complete normative inventory also has a language-neutral NDJSON protocol
and a hosted Rust reference runner:

```sh
cargo run -p conduit-conformance -- audit conformance/v1/manifest.json
cargo run -p conduit-conformance -- requests conformance/v1/manifest.json
cargo run -p conduit-conformance -- reference conformance/v1/manifest.json
```

Third-party implementations consume the `requests` stream and submit matching
NDJSON to `check-results`; no Rust or repository-specific binding is required.
See
[`spec/013-conformance-harness-v1.md`](spec/013-conformance-harness-v1.md).

Conduit is released under the MIT license.
