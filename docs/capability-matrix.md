# Capability evidence matrix

This file is generated from `release/capabilities.json` by `cargo xtask release-gate --check`. A status is a claim about one layer only; it must not be promoted across columns.

| Capability | Contract | Reference model | Provider | Host resolvability | Exact binding | Runtime proof | Product presentation |
|---|---|---|---|---|---|---|---|
| `semantic-core` | proven | proven | not-applicable | not-applicable | proven | proven | not-applicable |
| `pure-node-production-slice` | proven | proven | available | proven | proven | proven | proven |
| `bounded-http-serving` | proven | proven | available | proven | proven | proven | available |
| `zenoh-transport` | proven | proven | available | proven | proven | proven | contract-only |
| `exact-packaging` | proven | proven | available | not-applicable | proven | proven | not-applicable |
| `reference-panel-catalog` | proven | proven | contract-only | unsupported | unsupported | unsupported | proven |

## Release claims

- `semantic-core`: The semantic kernel is allocator-free and independently covered by frozen conformance vectors. Evidence: `crates/conduit-core/tests/flow_policy_vectors.rs`.
- `pure-node-production-slice`: The checked pure-node panel runs through the same exact bounded scheduler contract in the CLI and browser worker. Evidence: `crates/conduit-web/tests/pure_node_proof.rs`, `crates/conduct/tests/runnability_inventory.rs`.
- `bounded-http-serving`: A checked-in panel serves one bounded plaintext loopback route; rustls TLS behavior is separately proven with opaque host handles. Evidence: `crates/conduit-http/tests/http_vectors.rs`, `crates/conduct/tests/runnability_inventory.rs`.
- `zenoh-transport`: Hosted loopback transport semantics are executable; this is not a public deployment or complete hard-memory-bound claim. Evidence: `crates/conduit-zenoh/tests/transport_vectors.rs`.
- `exact-packaging`: Package bytes and identities are deterministic and bounded; trust metadata is validated but does not itself install or execute artifacts. Evidence: `crates/conduit-package/tests/package_vectors.rs`, `crates/conduct/tests/package_cli.rs`.
- `reference-panel-catalog`: Contract-only and illustrative panels remain visibly unavailable and cannot acquire run evidence. Evidence: `crates/conduct/tests/runnability_inventory.rs`.
