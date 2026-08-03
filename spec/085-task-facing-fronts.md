# Checked task-facing fronts

Status: candidate normative Patchbay presentation contract.

A task-facing front is a bounded presentation descriptor checked against one
current `SourceDocument`, semantic projection, candidate or exact plan,
runtime-owned action export and receipt, run, semantic result observation, and
terminal observation. It is not a second form schema, graph, callback surface,
execution API, or success channel.

## Descriptor and semantic ownership

The current pre-release descriptor has schema
`conduit.patchbay-task-front`, schema version `0`, and names one authored root.
It may supply only a name, purpose, bounded ordering/grouping, primary versus
advanced disclosure, labels, help, accessibility names, documentation, and a
finite semantic renderer-profile identifier.

Every control names exactly one of these authoritative sources:

- an explicitly exported composite parameter on the selected instance;
- an explicitly exported ordinary live input port; or
- an authorized site-binding slot already present in the checked
  configuration-layer projection.

The projector derives type, requiredness, default, current value origin,
sensitivity, editability, owner, persistence, and activation consequence from
those sources. Descriptor metadata cannot add a choice, default, callback,
private path, resource, authority, or implementation fact. Unknown descriptor
fields are rejected, so presentation metadata cannot weaken requiredness or
claim availability.

Live inputs remain typed ports and are read-only in configuration controls.
Missing exported instance parameters may be authored by the ordinary atomic
`SetConfig` source transaction; it cannot create a private or undeclared
field. Site bindings remain owned by their separate binding operation and
profile.

For a composite, a source `bind parameter = child.field` declaration is the
public configuration export. The definition body may defer that required child
field only because every instance must supply the inferred parameter under the
child field's exact type, sensitivity, mutability, and identity contract.
Descriptor metadata cannot make an unbound child field public.

## Action and result

The only current primary request is `run-exact-plan`. Exact plan presence does
not make that request available. The host/runtime must export a bounded action
for the current source identity, plan identity, nonzero plan epoch, and stable
operation identity, with an explicit `permitted` disposition. Denied, missing,
unavailable, binding-required, malformed, and stale exports remain visibly
non-actionable.

The presentation sends an asynchronous Patchbay request with a fresh request
identity and the exact exported identities. Runtime admission rejects stale
identities, colliding request identities, a second Start while one is pending
or active, and lifecycle controls not exported for the active run. Exact
duplicate requests replay the original receipt without dispatching a second
effect. Runtime assigns the run identity; a client cannot select one for
Start. Cancel and Drain require the exact active run and plan epoch.

Readiness is a Rust-owned state distinct from the descriptor and plan:
incomplete choices, checkable, binding required, unavailable, stale, denied,
start pending, ready, active, waiting, stopping, and terminal are projected
with bounded explanations. The descriptor does not carry a callback and the
presentation cannot report local success.

A result descriptor names one explicitly exported outgoing port. A displayed
semantic value is accepted only when a runtime-owned observation matches the
operation, request, exact plan, plan epoch, run, public port path, and semantic
type. Semantic status is one of succeeded, domain-rejected, or partial. A
stale or mismatched observation is rejected visibly.

Terminal state, cleanup state, and evidence-publication state are a separate
exact observation with the same causal identities. Runtime failure does not
erase a partial semantic result; terminal success does not manufacture one;
cleanup or evidence failure does not rewrite domain meaning. Stdout, console
prose, display-sink text, timeline position, and validation hints are not
semantic results. A terminal run without a matching semantic result says so.

## Modes and fallback

A usable checked descriptor opens the same workspace in Use. `Show how this
works` navigates that workspace to Build without changing source, controls,
configuration, choices, plan, or run. `Why did this happen?` navigates to
Inspect with the exact result subject. Neither operation starts or edits the
program.

No descriptor opens Build with `no-usable-task-front-declared`. A malformed,
private, stale, unsupported-renderer, or otherwise invalid descriptor opens
Build with `declared-task-front-is-invalid` and the Rust-owned explanation.
Renderer loss never disables headless authoring or execution.

Tour and self-hosted Patchbay consume the same serialized Rust view model.
The front hides source, private machinery, raw evidence, hashes, and opaque
handles in Use while keeping Show how and Why available. Primary controls,
action, and result fit a narrow viewport at 200% zoom; native labels, focus,
status output, forced colors, and reduced motion remain usable.

## Required invariants

- **TFR-001:** controls and results address explicit public semantic exports
  only; private reach-through fails closed.
- **TFR-002:** requiredness, defaults, types, choices, value origin,
  sensitivity, ownership, persistence, and activation are authoritative facts,
  not descriptor claims.
- **TFR-003:** instance configuration, live input, site binding, runtime state,
  and semantic result remain distinct identities and operations.
- **TFR-004:** renderer profiles are finite type-registry facts; malformed or
  wrong-type profiles invalidate the front.
- **TFR-005:** one primary action requests an explicitly exported and permitted
  exact-plan operation; plan presence alone is never actionable.
- **TFR-006:** requests and receipts retain exact operation, request, source,
  plan, epoch, and run identity; duplicates never dispatch twice.
- **TFR-007:** semantic results require operation, request, exact plan, epoch,
  run, port, and type identity; console output is never promoted into a result.
- **TFR-008:** semantic result, terminal state, cleanup, and evidence
  publication remain distinct observations and failure domains.
- **TFR-009:** no or invalid front produces an explained Build fallback.
- **TFR-010:** Use, Build, and Inspect preserve the same source, logical graph,
  plan, run, authority, and evidence resources.
- **TFR-011:** Tour and self-hosted surfaces serialize the same checked model
  and remain keyboard and high-zoom usable.
- **TFR-012:** bounds cap descriptors, action ledgers, controls, choices,
  result details, warnings, and text; an oversized front fails without partial
  projection.

Rust protocol and web tests cover zero/private/malformed fronts, required and
defaulted values, advanced controls, multiple instances, denial, duplicate and
colliding requests, cancellation, domain rejection, partial results, runtime
failure, cleanup/evidence failure, late prior-epoch results, invalid metadata,
and ordinary source editing. Tour browser tests
cover shared-surface identity, mode preservation, accessibility, high zoom,
exact task dispatch, and recognizable semantic and terminal outcome while the
raw console stays closed.
