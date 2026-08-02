# Checked task-facing fronts

Status: candidate normative Patchbay presentation contract.

A task-facing front is a bounded presentation descriptor checked against one
current `SourceDocument`, semantic projection, candidate or exact plan, run,
and semantic result observation. It is not a second form schema, graph,
callback surface, execution API, readiness model, or success channel.

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

## Action and result

The only current primary request is `run-exact-plan`. Its visible state is
derived from semantic availability, exact plan, and run state. The descriptor
does not carry a callback and the presentation cannot report local success.
The fuller readiness, request identity, stop, cleanup, and result lifecycle is
owned by issue #296.

A result descriptor names one explicitly exported outgoing port. A displayed
semantic value is accepted only when a runtime-owned observation matches the
exact plan identity, run identity, public port path, and semantic type. A
stale or mismatched observation is rejected visibly. Stdout, console prose,
timeline position, and validation hints are not semantic results. A terminal
run without a matching semantic result says so.

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
- **TFR-005:** one primary action requests the ordinary exact-plan operation;
  the front contains no callback or local-success path.
- **TFR-006:** semantic results require exact plan, run, port, and type
  identity; console output is never promoted into a result.
- **TFR-007:** no or invalid front produces an explained Build fallback.
- **TFR-008:** Use, Build, and Inspect preserve the same source, logical graph,
  plan, run, authority, and evidence resources.
- **TFR-009:** Tour and self-hosted surfaces serialize the same checked model
  and remain keyboard and high-zoom usable.
- **TFR-010:** bounds cap descriptor bytes, controls, choices, and text; an
  oversized front fails without partial projection.

Rust protocol and web tests cover zero/private/malformed fronts, required and
defaulted values, advanced controls, multiple instances, exact and stale
results, invalid metadata, and ordinary source editing. Tour browser tests
cover shared-surface identity, mode preservation, accessibility, high zoom,
run request, and honest terminal-without-result presentation.
