# Named node interfaces cookbook

Named node interfaces enable consumers and domain integration authors to depend on a stable, named capability boundary (`NodeInterfaceContract`) without coupling graph wiring or plan validation to a single concrete node species, primitive catalog entry, or composite definition.

## Contract layer decision matrix

Keep these identities distinct when modeling domain capability boundaries:

| Layer Identity | What It Defines | When To Use It |
|---|---|---|
| **TypeContract** | Domain-owned value meaning, structure, and compatibility algebra. | To define valid payload values (e.g., `speech/transcript`, `conduit/text.utf8`). |
| **PortContract** | One directional live boundary, combining type, presence, cardinality, delivery, temporal, terminal, sensitivity, and flow rules. | To specify single port requirements or export definitions. |
| **NodeInterfaceContract** | A named, required node boundary composed of finite port members and non-port facts. | To define reusable capability interfaces (e.g., `speech/recognizer`, `conduit/stream-sink`). |
| **NodeContract** | One concrete primitive or composite semantic node contract. | To describe the actual public boundary of a concrete node or composite module. |
| **Implementation** | Host-executable step logic selected for a concrete node contract. | To execute runtime steps on hosted systems (Linux, RP2040, WASM). |
| **Host-Service Trait** | Low-level host platform observation or hardware interface. | For host infrastructure (HIL, Zenoh, Web sockets), above graph semantics. |

## Authoring node interfaces and claims

State: **illustrative/unavailable**. The snippet demonstrates contract syntax;
it does not identify an installed speech implementation.

Declare reusable named interfaces using the `interface` keyword in `.panel` source files:

```panel
panel 2

interface speech/recognizer {
  input audio : conduit/text.utf8
  output final : conduit/text.utf8
  output partial : conduit/text.utf8 optional
}

# Primitive node declaring interface satisfaction
node asr_primary : conduit.std/stdout implements speech/recognizer

# Composite node declaring interface satisfaction via transparent exports
node speech_pipeline implements speech/recognizer {
  node sink : conduit.std/stdout
  export input sink.in as audio
  export output sink.in as final
}
```

## Key satisfaction rules

1. **Directional Assessment**: Interface satisfaction is verified by `conduit-core` using `assess_node_interface`. Inputs match inputs, outputs match outputs.
2. **No Implicit Adapters**: Conduit never auto-generates adapter nodes, coercions, or queues. If a candidate node's port shape or direction differs from the interface, an explicit adapter composite must bridge the gap.
3. **Optional Members**: An optional interface member (marked `optional`) may be absent from the candidate node. If present, it must satisfy all directional port rules exactly.
4. **Extra Ports Allowed**: Concrete candidate nodes may offer additional public ports beyond what the interface requires. Extra ports do not create hidden interface members or security grants.
5. **Exact Evidence Retention**: Verified proofs are pinned into `LoweredSourceV4` and `ExecutionPlan` structures and can be inspected via `conduct explain` and `conduct inspect`.

## CLI Workflow

Check and explain interface satisfaction using the `conduct` CLI tool:

```sh
# Verify syntax and lower interface claims
conduct check examples/interface-consumer.panel

# Inspect lowerings, exact execution plans, and interface proofs
conduct explain examples/interface-consumer.panel
```

## Failure Diagnostics

When an `implements` claim fails directional satisfaction or references an unavailable interface, lowering rejects the panel with diagnostic `CND-LWR-013`:

```sh
conduct check examples/interface-diagnostic-failure.panel
# Diagnostic CND-LWR-013: interface contract `speech/recognizer` satisfaction failed: node-interface-required-member-missing
```

## Useful Focused Checks

```sh
cargo test -p conduit-runtime --test interface_adoption_vectors
cargo test -p conduit-compile
```
