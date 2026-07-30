# Conduit architecture and evidence roadmap

This document describes the current repository, not historical issue state.
The mechanically checked capability source is
[`release/capabilities-v1.json`](../release/capabilities-v1.json), rendered as
the [capability evidence matrix](../docs/capability-matrix.md). A closed issue,
crate name, screenshot, specification, or candidate manifest is not evidence
that a capability runs.

## Production dependency graph

```text
.panel source
  -> conduit-panel parsing and typed lowering
  -> conduit-runtime semantic registry
  -> conduit-compile exact candidate, host, authority, and budget selection
  -> immutable conduit-core execution plan
  -> exact installed artifact and implementation bindings
  -> DeterministicExecutor
  -> typed bounded execution evidence
  -> conduit-patchbay / Tour presentation projection
```

The CLI and browser worker enter this graph through different host adapters but
execute the same exact-plan scheduler contract. `ResolvedPanel::run` is a
compatibility/demo boundary and is not production evidence. Presentation never
owns source semantics, planning, provider availability, or run truth.

## Current proved slices

| Slice | Current evidence | Claim boundary |
|---|---|---|
| Semantic core | `conformance/v1/manifest.json`, `conduit-core` vector tests | Allocator-free semantic and execution contracts, not a hosted provider |
| Pure hosted/browser node slice | `conformance/c5/pure-node-v1.panel`, `conduit-web/tests/pure_node_proof.rs`, `conduct/tests/runnability_inventory.rs` | Literal, uppercase, stdout, exact plan, bounded scheduler, evidence, and projection |
| Patchbay editing | `conduit-patchbay/tests/protocol.rs`, `tour/tour.spec.mjs` | Revisioned typed intents over Rust projections; presentation failure remains headless |
| HTTP serving | `conformance/c5/http-serving-v1.json`, `conduit-http/tests/http_vectors.rs` | One bounded loopback route plus backend/TLS conformance; not public deployment |
| Zenoh transport | `conformance/c5/zenoh-transport-v1.json`, `conduit-zenoh/tests/transport_vectors.rs` | Hosted loopback and exact transport selection; physical/constrained deployment remains separate |

Every checked-in panel has an independent state and executable or rejection
proof in [`examples/runnability-v1.json`](../examples/runnability-v1.json).
Lesson completion does not upgrade that state.

## Availability is layered

The following states are not interchangeable:

1. a semantic contract exists;
2. a reference model or conformance oracle exists;
3. an executable provider is installed;
4. current host facts make it resolvable;
5. an exact plan binds its artifact, authority, and budget;
6. a run produced typed evidence;
7. a product presents that evidence.

The release gate rejects a matrix that omits a layer, a runnable example whose
proof is only a rejection/check, missing release metadata, or a public runtime
claim without an executable evidence path.

## Recovery order

The recovery spine is dependency ordered:

1. quarantine placeholders and separate availability;
2. exact standard identities, manifests, and installed bindings;
3. exact plan to `DeterministicExecutor` production execution;
4. pure-node CLI/browser/evidence/Patchbay proof;
5. authoritative Patchbay projections and typed edits;
6. honest Tour/reference runnability;
7. genuine bounded HTTP host-service proof;
8. release and roadmap claims derived from executable evidence;
9. only then revisit distributed/brownfield adoption.

Useful partial work closes only with the
[accepted-slice disposition](../.github/ISSUE_TEMPLATE/accepted-slice.md).
Residual requirements must move atomically to a linked open issue with their
acceptance criteria and negative fixtures.

## Release boundary

`cargo xtask release-gate --check --output target/release-evidence.json`
validates the checked capability source, generated matrix, runnable examples,
version, changelog, license, repository identity, and supported-host boundary.
The emitted artifact records the exact Git commit plus hashes of the claim and
runnability inputs. CI publishes it only after workspace tests.

Current supported-host language is deliberately narrow:

- Linux hosted and the browser dedicated worker are tested profiles.
- `thumbv6m-none-eabi` is a conditional allocator-free build/conformance
  profile.
- Physical RP2040 HIL is unsupported unless the separately triggered hardware
  workflow produces its required report.
- No current claim covers public-Internet deployment, firewall/certificate
  provisioning, multi-tenant confinement, hazardous actuation, or certified
  safety.

## C6/C7 brownfield entry gates

Tongues and Netherwick adoption stays last. Work may begin only after:

- the recovery spine above is green on the exact integration commit;
- repository-owned contracts remain domain-neutral;
- the target repository supplies an explicit provider inventory and runnable
  baseline;
- authority, cancellation, resource, evidence, and supported-host boundaries
  have negative fixtures;
- required physical HIL is available and produces exact artifact/firmware/run
  identities;
- residual work has focused owners rather than being hidden in migration prose.

Until those gates are met, C6/C7 remain design or contract work, not runnable
Conduit capability claims.
