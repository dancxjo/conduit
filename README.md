# Conduit

Conduit is a portable execution substrate for composing heterogeneous software
and hardware from typed, inspectable nodes.

It is meant to make systems design feel more like wiring a modular instrument
than embedding one large application: components have explicit interfaces,
connections have declared behavior, and a complete arrangement can be checked,
explained, executed, and eventually replaced without losing its identity.

Conduit is not an operating system and it is not tied to one domain. It is a
common composition layer that can run above Linux, WebAssembly, embedded
firmware, and distributed hosts. A speech pipeline, a robot behavior, a web
server, or a sensor network can all be described using the same underlying
model.

## Experimental safety status

Conduit is experimental. It has no stable security-supported release and is
not ready for safety-critical, autonomous, multi-tenant, production
infrastructure, fleet enrollment/provisioning, network-boot orchestration, or
hazardous physical deployment.

Its general composition model can combine discovery, membership, authority,
artifacts, persistence, distributed placement, plan transitions, and physical
effects. Finite local operations are not sufficient containment when a
sequence of operations can expand a system across hosts or epochs. The project
therefore treats non-escalation, persistent cumulative budgets,
administrative-plane separation, safe distribution defaults, whole-plan hazard
analysis, independent inhibits, and adversarial conformance as architectural
work—not deployment options to add later.

The public safety program is tracked by
[#92](https://github.com/dancxjo/conduit/issues/92). See
[Safety, deployment boundaries, and stewardship](docs/safety-and-stewardship.md)
before attempting consequential use. Report unpublished vulnerabilities
privately according to [SECURITY.md](SECURITY.md).

Specifications, conformance fixtures, exact plans, signatures, provenance, and
evidence improve inspectability; none alone is a security certification,
sandbox guarantee, or authorization to deploy. Dangerous administrative
providers and convenience tooling should remain absent from reference
distributions until their containment contracts and negative tests exist.

## The model

The runtime ontology is deliberately small:

- a **node** performs a semantic computation;
- a typed, directional **port** forms its boundary;
- a bounded **cord** connects an output port to an input port;
- a **panel** is editable source describing an arrangement of nodes and cords.

A panel can expose its own typed ports and become a reusable composite node.
Composition is therefore recursive without introducing a special runtime kind
of “panel”: at its boundary, every composite remains a node.

This is more expressive than assuming that every component is only stdin,
stdout, and stderr. Unix streams remain useful, but Conduit can represent
multiple typed inputs and outputs, grouped ports, bounded replication, host
services, and other domain-specific contracts transparently.

## One semantic model, many hosts

Conduit separates what a node means from how it is implemented. The same
semantic capability may be provided by Rust, native code, WebAssembly, Python,
remote services, or embedded firmware.

Hosts advertise capabilities and resources; implementations declare their
requirements. Conduit resolves compatible implementations into an exact plan,
but it does not provision or administer the host.

That distinction lets Linux and a Pico W participate in the same conceptual
system without pretending they have the same resources. Linux may host a
network service or a large model; a Pico W may host a compact control pipeline.
They share panel semantics and contracts, while their available
implementations and capabilities remain honest.

The portable core is `#![no_std]`-capable. The embedded goal is a compact
compiled pipeline image executed by a small runtime, not a miniature Linux
distribution.

Replicated composite pools are finite schema-16 plan populations rather than
dynamic graph mutation. See the
[replicated pool cookbook](docs/cookbook-replicated-pools.md) for exact
admission, identity, reservation, cleanup, and generation-overlap guidance.

## Working with panels

The `conduct` command checks, explains, and runs panels:

```sh
cargo run -p conduct -- examples/hello.panel
cargo run -p conduct -- --check examples/hello.panel
cargo run -p conduct -- --explain examples/hello.panel
cat examples/hello.panel | cargo run -p conduct -- -
```

`--run` is the default. A `.panel` is source, not bytecode, ELF, or a
universal package. Compilation means validation, resolution, and lowering to
an exact execution plan; packaging and code generation are separate
operations.

Exact compilation is an additive, explicit-input workflow:

```sh
conduct compile --input compile-input.json --format=json panel.panel
```

The input document pins the complete module closure, selected root, finite
semantic catalog snapshot, manifests, fresh host reports, realm/passport
policy, authority observations, time, schema-16 pool runtime and generation
bindings, and finite budgets.
Compilation performs no discovery, provisioning, artifact fetch, grant
acquisition, loading, or execution. `conduct package create` builds a
deterministic thick or thin content-addressed envelope from a sealed manifest
and explicit `--blob SHA256=PATH` arguments. `conduct package verify` applies
an explicit JSON trust policy to external JSON signature-verification
observations; declared signature bytes alone never imply trust. `conduct
package extract` validates the envelope and writes only digest-derived paths.
`conduct inspect --type=package` reads the same envelope without fetching,
extracting, loading, or executing contained objects.

Primary values stay on stdout. Diagnostics and interactive status use stderr:

```sh
conduct --check --diagnostic-format=json panel.panel
conduct --check --color=never --verbose-diagnostics panel.panel
conduct --check --format=json panel.panel
conduct --explain --format=json panel.panel
conduct --check --compile-input compile-input.json panel.panel
conduct --explain --compile-input compile-input.json panel.panel
conduct --run --format=ndjson panel.panel
```

Diagnostic format is `human` or `json`; color is `auto`, `always`, or `never`.
Automatic color honors `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, terminal
attachment, and `TERM=dumb`. Redirected stderr receives no status animation or
cursor control, and a downstream closed stdout pipe is treated as normal
completion. Primary `--format` is separate: check/explain use finite
`conduit.result/v1` JSON, while run uses ordered, bounded
`conduit.run/v2` NDJSON. Its `channel_chunk` records preserve compatibility
stdout/stderr bytes without claiming that implementation write boundaries are
semantic values. Executor-owned immutable evidence uses a distinct bounded
`execution_event` record and the plan-visible normative Resonance profile.
`--quiet` suppresses status but never primary values or diagnostics; `-v` adds
general terminal status detail and remains distinct from
`--verbose-diagnostics`.

Safely inspect supported artifacts without executing them:

```sh
conduct inspect examples/hello.panel
conduct inspect --format=json conformance/v1/manifest.json
conduct inspect --type=evidence run-events.ndjson
```

Inspection uses frozen markers, fixed byte/record/depth/module limits, and
structural redaction. It performs no network access, provider discovery,
secret resolution, authority acquisition, dynamic loading, or artifact
execution. Source, lowering, exact plans, evidence, diagnostics, content
digests, and inspection reports retain distinct identities.

## Why the runtime is opinionated

Conduit treats runtime behavior as part of the system’s meaning rather than as
an incidental library detail:

- live flow is bounded, with explicit pressure, delivery, and loss policy;
- cancellation and terminal behavior are deterministic;
- queues use caller-provided fixed storage at the allocator-free boundary;
- live execution preallocates exact cord storage and declared scheduler
  overhead, then uses reasoned round-robin wake decisions without hidden
  channels;
- execution plans pin implementation, artifact, host, resources, authority,
  bounded execution profile, and provenance before start;
- nonblocking node steps use executor-mediated input leases, output
  reservations, exact wake interests, and finite retained/scratch/host bounds;
- diagnostics and evidence can be durably associated with an exact run and plan;
- typed supervision keeps expected domain outcomes, runtime terminal causes,
  and pre-run diagnostics separate while allowing only finite, exact-plan
  admitted recovery decisions;
- composite definitions expose only explicit typed ports and explain both their
  logical and flattened primitive forms.

This makes a pipeline something that can be inspected and reasoned about, not
just something that happens to execute.

## Direction

The repository is moving from a strong executable foundation toward a general
system for building long-lived, evolvable arrangements.

Important parts of that direction include:

- production distributed carriers, including Zenoh, behind the implemented
  plan-v9 transport-neutral session and bounded backend boundary;
- composable HTTP/HTTPS serving through host backends;
- standard node libraries and host-service interfaces;
- port groups and bounded composite replication;
- capability-aware implementation selection;
- independent conformance implementations and language-neutral protocols;
- swappable node behavior.

A swappable node keeps its semantic identity while its implementation or plan
epoch changes. Replacements may be cold, quiescent, or stateful. Live changes
therefore require a declared handoff boundary, optional state transfer, and
atomic port rebinding.

The goal is not merely to make pipelines convenient. It is to provide a
stable language for expressing how capabilities become systems—across
processes, machines, runtimes, and devices—while keeping the boundaries,
constraints, evidence, and substitutions visible.

## Status

This repository contains the first executable Plan C foundation. It remains a
research and development system: current code and candidate specifications
must not be represented as a supported security profile, autonomous deployment
platform, or certified safety boundary.

The implemented foundation includes:

- `conduit-core`: allocator-free contracts, canonical semantic hashes,
  opaque type references, port/config schemas, bounded flow-policy state
  machines, host-neutral implementation steps/transactions, realm-aware
  distributed-cord sessions, compatibility, and plan validation;
- `conduit-panel`: the versioned `.panel` grammar, lossless CST/source AST,
  module resolution, reusable definitions, groups, and finite pools;
- `conduit-diagnostics`: owned structured diagnostics, lossless JSON, guarded
  source fixes, cross-file spans, and concise/verbose terminal rendering;
- `conduit-inspect`: hosted bounded, marker-only, non-executing artifact
  validation and value-safe reports;
- `conduit-runtime`: a hosted registry, typed-config resolver, explainer,
  one-shot executor, deterministic bounded streaming executor, and
  native/message step-binding examples, plus a carrier-neutral distributed
  backend boundary and bounded fault reference;
- `conduit-http`: domain-owned HTTP types and ordinary serving composites,
  exact bounded host selection, deterministic routing/session faults, and real
  Linux TCP/rustls backends without adding HTTP concepts to the core;
- `conduct`: the Unix command-line interface.

The initial runtime includes intentionally small proof handlers for literal
UTF-8 text, stdin, uppercase transformation, stdout, and stderr. They establish
the complete parse → check/explain → resolve → run path without pretending to
be the final standard library.

Python is used as an independent conformance oracle, not as a privileged
runtime path. Tongues and Netherwick may supply domain contracts; Conduit does
not contain speech- or robot-specific types.

## Development

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
just cli-assets-check
python3 conformance/c1/verify_canonical_v1.py
cargo check -p conduit-core --no-default-features \
  --target thumbv6m-none-eabi
```

Run `just cli-assets` after changing the shared `conduct` command model. It
reproducibly updates Bash, Zsh, Fish, PowerShell, and Elvish completions plus
the generated manual page; CI rejects checked-in drift.

The complete normative inventory also has a language-neutral NDJSON protocol
and a hosted Rust reference runner:

```sh
cargo run -p conduit-conformance -- audit conformance/v1/manifest.json
cargo run -p conduit-conformance -- requests conformance/v1/manifest.json
cargo run -p conduit-conformance -- reference conformance/v1/manifest.json
```

Third-party implementations consume the `requests` stream and submit matching
NDJSON to `check-results`; no Rust or repository-specific binding is
required. See
[`spec/013-conformance-harness-v1.md`](spec/013-conformance-harness-v1.md).

Conduit is released under the MIT license.
