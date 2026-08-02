# Patchbay structural lenses and intent modes

Status: candidate normative Patchbay presentation contract.

This contract defines navigation over one authored panel, its logical composite
instances, enclosing graph, exact realization, run, and evidence. Navigation is
stored in `PresentationDocument`; it never creates another graph, runtime panel
kind, lifecycle state, provider observation, or authority path.

## Identity and ownership

The presentation keeps these independently identified resources visible:

| Resource | Owner | May change it |
| --- | --- | --- |
| `SourceDocument` and panel definition | authoring transaction | source edit |
| logical composite instance and authored configuration | enclosing source | source edit |
| external semantic cords | enclosing source | connect/disconnect source edit |
| internal cords and export mappings | reusable definition | definition source edit |
| mode, lens, selection, collapse, layout, viewport | `PresentationDocument` | presentation transaction |
| implementation, host, resource, grant, lease, bounds | exact plan | resolution or #57 transition |
| current state | run epoch | admitted runtime control |
| portable state | checkpoint | checkpoint protocol |
| history | evidence stream | immutable evidence append |

An external cord names the enclosing panel as owner and may cross a composite
boundary only through an explicitly exported public port. Opening Context does
not make a private child port addressable. Opening Inside does not move an
internal cord into the enclosing source.

## Structural lens axis

The selected lens answers which boundary is being viewed:

- `at-rest`: inspect an unloaded definition/capsule contract, documentation,
  imports, defaults, fixtures, and observed availability without fetching,
  installing, resolving, granting, acquiring, or starting;
- `face`: the logical instance boundary derived only from its exported typed
  directional ports and explicitly exported configuration/control contracts;
- `inside`: authored children, definition-owned internal cords, adapters,
  supervision, bounds, bindings, and export mappings;
- `context`: the instance within its enclosing graph, including
  enclosing-panel-owned semantic cords and separately identified exact-plan
  realization bindings;
- `configure`: the explicit configuration/state layers described below.

Changing this axis changes presentation revision and identity only.

## Intent mode axis

The mode is independent of the structural lens and Logical/Expanded topology:

- `use` presents a declared task front, its primary controls/action, readiness,
  material warnings, semantic result, and honest terminal outcome. Source,
  internals, hashes, opaque handles, allocations, watermarks, and raw evidence
  remain hidden until requested.
- `build` presents source, logical topology, candidate diagnostics, definition
  internals, exports, semantic configuration, and candidate check/preview.
- `inspect` is read-only and presents exact contracts, resolved paths, active
  and candidate plan facts, realization bindings, lifecycle, pressure,
  causation, and authorized bounded evidence.

`Use` is selected initially only when the checked task-front contract in
specification 083 returns a usable front. Otherwise the opening mode is `Build`
with `no-usable-task-front-declared` or `declared-task-front-is-invalid`;
presentation must not invent a form. Such a fallback cannot be navigated into
Use until a new workspace is opened with a usable checked descriptor.

`Show how this works` changes to Build while retaining the selected subject.
`Why did this happen?` changes to Inspect while retaining that exact subject.
Neither action resolves, edits, starts, stops, grants, or acquires anything.

## Presentation transactions

The current `conduit.patchbay` protocol adds these ordinary presentation-only
operations alongside node movement:

- `Navigate { mode, lens, topology }`;
- `SelectSubject { subject }`;
- `SetCollapsed { node_id, collapsed }`;
- `SetViewport { x, y, zoom_basis_points }`.

Mode, lens, topology, selection, collapse state, positions, viewport, opening
reason, document identity, and revision are all hashed into presentation
identity. Zoom is an integer from 2,000 through 30,000 basis points (20%-300%).
Subject paths are bounded, parser/projector-authored paths. An invented or stale
path fails atomically with `CND-PBY-013`.

`inspect_at_rest` and the WASM `patchbay_inspect_at_rest` entry point parse and
project an unloaded source without consulting a registry or opening a session.
Its operation record explicitly reports fetch, installation, resolution,
authority/resource acquisition, and run start as false; provider availability
is `not-observed`, not silently treated as absent. It returns no plan, run, or
evidence shape.

A presentation transaction preserves source bytes and semantic identity,
descriptor identity, exact plan, run/checkpoint/evidence, task choices, and
semantic result. A later source transaction may clear a selected subject that
no longer exists, but cannot reinterpret the old subject as a new identity.

## Configure layers

Configure does not expose a universal mutable `config` or `state` bag. The
Rust projection emits separate finite layers and every layer names:

- owner;
- persistence boundary;
- revision or epoch;
- sensitivity policy;
- mutability;
- activation consequence; and
- bounded fields safe for that projection.

The current layers distinguish definition defaults, instance-authored source
configuration, live typed input ports, exact-plan implementation/host/resource
bindings, user/workspace presentation preferences, current run state, and
immutable evidence. Checkpoint state remains a separate layer when present.
Opaque handles and secret values remain redacted. Live inputs remain ordinary
ports and cannot be edited as configuration.

## Presentation and accessibility

Canvas selection and the structured port/cord lists consume the same Rust
semantic paths. Mode and lens controls are keyboard buttons with `aria-pressed`
state and a live ownership/status explanation. The full semantic port name and
direction remain available in accessible text. Visual state is never the only
way to distinguish a semantic cord from an exact-plan realization binding.

The default Use surface reserves one ordinary viewport, including at 200%
zoom, for primary controls, primary action, readiness/material warning, and
semantic result. Deep implementation facts remain in Inspect. This is an
information budget, not permission to hide partial commits, cleanup failures,
authority requirements, or unsupported outcomes.

## Required invariants

- **PBL-001:** definition, instance, enclosing source, presentation, plan, run,
  checkpoint, and evidence never become one mutable object.
- **PBL-002:** internal cords are definition-owned; external cords are
  enclosing-panel-owned and cross exported public ports only.
- **PBL-003:** Face is derived from semantic exports and reveals no private
  implementation detail.
- **PBL-004:** Context distinguishes semantic cords from provider, host,
  transport, resource, grant, and lease realization facts.
- **PBL-005:** every projected configuration field exposes its owner,
  persistence, revision, sensitivity, mutability, and activation consequence.
- **PBL-006:** selection uses authoritative stable paths in canvas, structured
  navigation, source, plan, run, and evidence views.
- **PBL-007:** Use/Build/Inspect, structural lens, and Logical/Expanded remain
  three independent presentation dimensions.
- **PBL-008:** no task front means explained Build fallback, never an empty or
  client-invented Use form.
- **PBL-009:** mode/lens navigation cannot trigger selection synthesis,
  resolution, authority, source editing, plan transition, or execution.
- **PBL-010:** renderer loss cannot remove headless authoring or execution
  control, and screen-reader navigation remains available.

The normative fixtures are in
`conformance/c8/patchbay-protocol.json`; Rust protocol tests execute the state
and identity rules, while Tour browser tests execute visible keyboard,
screen-reader, preservation, and information-budget behavior.
