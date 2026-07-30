# Cross-host provider and extension conformance version 1

Status: candidate normative C5 contract. The fixture
`conformance/c5/cross-host-provider-conformance-v1.json` exercises HCF-001
through HCF-009.

## Separate facts

A host conformance profile has one exact identity, a material host class, an
executable or describe-only mode, mandatory execution facts, an optional
provider inventory, and namespaced extension descriptors. These collections
remain separate. An empty optional-provider inventory is valid. Publishing a
descriptor means only that the host understands it; discovery installs no
code, initializes no provider, grants no authority, enrolls no host, and makes
no provider selectable (HCF-001, HCF-009).

Linux hosted, browser/WASM, constrained firmware, deterministic-test, and
describe-only profiles use the same host-neutral structures. A describe-only
profile is never executable. Known-unsupported, absent, linked, linked but
uninitialized, currently available, lost, and stale are distinct outcomes
(HCF-002, HCF-003).

## Complete provider chain

An exact binding retains this chain:

`required contract → offered facets → satisfaction proof → behavioral
conformance result → provider bundle → current host observation → exact
implementation/artifact/adapter binding`.

The structural satisfaction proof is the complete directional proof from
specification 027. Labels, port shape, source-language traits/interfaces, JSON
fields, ABI resemblance, successful process exit, or discovery order are not
proof. A behavioral conformance result additionally pins the exact
implementation, artifact, adapter boundary, profile, fixture suite, offered
facets, finite bounds, outcome, and validity interval (HCF-004, HCF-006).

Native Rust, supervised non-Rust processes, browser/WASM, and firmware/FFI
providers may satisfy the same semantic contract. Their boundary, adapter,
artifact, host, resource, cancellation, isolation, and trust facts remain
different. Missing interpreter/runtime, protocol mismatch, wrong artifact,
failed fixtures, stale results, non-cancellable work, and a hidden foreign
queue fail distinctly (HCF-007, HCF-008).

## Custom extensions and adapters

Types, nodes, implementations, and adapters are open namespaced descriptor
pins. A custom type crosses a host boundary only when its exact descriptor
identity matches. Different type identities require one explicitly requested,
profile-published adapter whose exact identity is also the adapter tested by
the behavioral conformance result. No adapter is inferred or inserted
(HCF-005).

Provider loss changes current resolvability and can terminate an active
bounded run, but it does not mutate the contract, profile, source, plan, or
conformance-result identities. Re-resolution constructs a new exact binding;
it never edits an active plan.

## Inspection and presentation

`conduit.host-conformance-report/v1` is a bounded presentation document.
`conduct inspect` reports mandatory host facts, optional providers, extensions,
and the complete exact binding chain as separate typed references. Inspection
is read-only. Patchbay projects the same categories and finite execution
bounds without becoming authority or evidence.

## Stable outcomes

- `CND-HCF-001`: unsupported schema
- `CND-HCF-002`: malformed or inconsistent profile
- `CND-HCF-003`: describe-only profile presented as executable
- `CND-HCF-004`: optional provider absent
- `CND-HCF-005`: contract known but unsupported
- `CND-HCF-006`: linked provider uninitialized
- `CND-HCF-007`: provider lost
- `CND-HCF-008`: provider observation stale
- `CND-HCF-009`: provider observation/profile mismatch
- `CND-HCF-010`: behavioral conformance failed or unsupported
- `CND-HCF-011`: behavioral conformance result stale
- `CND-HCF-012`: conformance implementation/artifact/profile mismatch
- `CND-HCF-013`: incomplete, invalid, or wrong-operand satisfaction proof
- `CND-HCF-014`: direct custom type relation incompatible
- `CND-HCF-015`: required explicit adapter absent
- `CND-HCF-016`: adapter not published or not the tested exact adapter

## Requirements

| ID | Requirement |
|---|---|
| HCF-001 | Share one bounded host-neutral profile and binding model across materially different host classes |
| HCF-002 | Keep executable, describe-only, empty, absent, and known-unsupported profiles distinct |
| HCF-003 | Keep static inventory separate from current initialized, available, lost, and stale observations |
| HCF-004 | Require a complete satisfaction proof; never infer conformance from labels, shape, language, ABI, or process exit |
| HCF-005 | Require exact custom-type identity or one explicit exact tested adapter |
| HCF-006 | Pin behavioral conformance to exact fixtures, facets, profile, implementation, artifact, adapter, validity, and finite bounds |
| HCF-007 | Preserve native, supervised-process, WASM/browser, and firmware/FFI boundary facts without changing semantic meaning |
| HCF-008 | Bound work, foreign queues, memory, cancellation, and evidence, with terminal failures when exceeded |
| HCF-009 | Keep inspection and discovery read-only and incapable of installation, initialization, enrollment, authority, or selection |
