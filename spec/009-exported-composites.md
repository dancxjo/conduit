# Exported composite nodes current form

Status: normative current contract

Composite algebra schema marker: `0`

## Purpose and identity boundaries

A composite is an ordinary node definition whose implementation is an
assemblage of child node instances and bounded cords. It has one complete
`NodeContract` at its boundary. There is no runtime `Panel` variant and no
composite-specific scheduler.

These identities remain distinct:

- the editable `.panel` document;
- a composite definition and its semantic boundary contract;
- each logical instance path;
- the lowered primitive execution topology;
- the exact execution plan; and
- run evidence and presentation metadata.

Changing a child, cord, export, or semantic parameter binding changes
definition identity even when its boundary remains substitution-compatible.
Plan identity is specified by issue #11; this specification does not hash
editable source or presentation.

## Source model

The executable seed grammar adds reusable definitions:

```text
example/upper-line{
    source: std/literal
    upper: text/uppercase
    source.value > upper.text
    export text > = upper.text
    bind value = source.value
}

line: example/upper-line { value = "hello" }
sink: display/text
line.text > sink.text
```

Definitions may contain primitive or composite child instances. The top-level
document is the authored root assemblage; it is not another runtime kind.
Issue #14 owns imports, modules, richer declarations, and the final grammar.

Local instance IDs exclude `.` and `/`; definition IDs remain qualified
semantic IDs. A nested logical path joins local IDs with `/`, for example
`root/inner/worker`. The current flat plan spelling replaces separators with
`.` only after local-ID validation, so the mapping is injective and stable.

## Transparent exports

An export maps exactly one boundary input to one immediate child input, or one
boundary output to one immediate child output. Direction cannot change.
Every boundary port has exactly one mapping and duplicate boundary names are
invalid.

The boundary and mapped `PortContract` may differ only in local port ID. They
retain exactly:

- TypeContract reference;
- presence and connection cardinality;
- value cardinality and delivery;
- temporal and terminal contract;
- sensitivity;
- loss acceptance and other flow constraints.

Thus an exported composite can substitute wherever its complete boundary
contract is accepted. Exporting adds no adapter, queue, loss, authority,
declassification, or terminal behavior.

Outside cords resolve only against explicit exports. A reference such as
`root.child.result` cannot cross `root` unless `root` exports that port.
Internals may be shown in expanded diagnostics but visibility does not grant
patchability.

## Parameters and configuration

`bind parameter = child.field` maps an explicit composite configuration
parameter to child configuration. A parameter may feed several child fields
when all mappings retain the complete config-field semantics. Bindings never
create ports.

Missing parameters, unknown parameters, dangling bindings, duplicate exact
bindings, conflicting authored child values, and incompatible field contracts
are rejected before execution. Values remain subject to the child contract's
type, default, mutability, sensitivity, redaction, and identity rules.

Parameterized templates used by bounded replicated pools remain ordinary
composite definitions. Issue #44 adds finite admission/pool and correlation
facts; it does not permit arbitrary graph mutation.

## Recursive validation and lowering

Validation is deterministic and rejects:

- duplicate definition or child identities;
- recursive direct or indirect definition cycles, including unused
  definitions;
- duplicate, missing, dangling, wrong-direction, or incompatible exports;
- dangling internal cords;
- dangling or incompatible config bindings; and
- cross-instance references that bypass exports.

Lowering recursively replaces a composite instance with its child topology.
External and parent cords are rewritten to mapped primitive endpoints;
internal FlowPolicy values are copied exactly. DFS source order determines
expanded node/cord order, never registry or hash-map order.

The lowerer retains each logical composite path, definition identity, export
name/direction, and final expanded endpoint. `conduct --explain` shows both:

- a logical view with authored root instances and export provenance; and
- an expanded view containing only primitive implementation selections and
  executable cords.

Both views describe one resolution. Choosing a view does not change source,
definition, plan, or run identity.

## Fan-out, lifecycle, and evidence

Fan-out through an exported output is identical to fan-out from the mapped
primitive output. Its complete connection cardinality and flow constraints
are validated after lowering, and every outgoing cord keeps its own finite
FlowPolicy.

Composite lifecycle derives recursively from children and boundary cords
under specification 008. Flattening therefore yields the same terminal,
cancellation, drain/abort, and supervision result as the logical view.
Evidence records stable logical and expanded paths so observation does not
erase the composite boundary.

## Portable core and hosted seed

`conduit-core` provides allocator-free borrowed `CompositeDefinition`,
`CompositeChild`, `CompositeExport`, `CompositeConfigBinding`, and
`InstancePath` contracts. It checks complete boundary equivalence and validates
definition dependency graphs using caller-provided marks.

The hosted seed parser/resolver owns strings and vectors, performs recursive
source lowering, and executes the resulting primitive graph. Domain behavior,
host selection, artifacts, and UI state remain outside the core.

## Diagnostics and fixtures

| Code | Meaning |
|---|---|
| `CND-CMP-001` | invalid or duplicate definition/instance identity |
| `CND-CMP-002` | duplicate or missing export/binding |
| `CND-CMP-003` | dangling cord, export, or binding |
| `CND-CMP-004` | boundary contract or parameter binding is incompatible |
| `CND-CMP-005` | recursive or unknown composite definition dependency |
| `CND-CMP-006` | attempted access bypasses an instance boundary |
| `CND-CMP-007` | composite parameter is missing, unknown, or conflicting |
| `CND-CMP-008` | allocator-free validation scratch is insufficient |

`conformance/c2/composite.tsv` freezes one-level and nested exports,
fan-out, parameter provenance, and every required negative classification.
`examples/composite-uppercase.panel` is an authored
**illustrative/unavailable** example. The canonical hosted run currently
rejects it with `CND-CMP-004`; it must not be presented as executable until
that exact path has conformance evidence.

## Normative requirements

| ID | Obligation |
|---|---|
| CMP-001 | Keep a composite an ordinary node with one complete boundary contract |
| CMP-002 | Retain every PortContract fact across an export |
| CMP-003 | Permit external patching only through explicit exports |
| CMP-004 | Reject direct and indirect definition recursion |
| CMP-005 | Preserve deterministic logical and expanded instance paths |
| CMP-006 | Copy every internal and external FlowPolicy exactly during lowering |
| CMP-007 | Keep configuration bindings explicit and separate from ports |
| CMP-008 | Preserve flattened fan-out and lifecycle behavior |
| CMP-009 | Retain export provenance for diagnostics and evidence |
| CMP-010 | Never introduce a Panel runtime kind or composite scheduler |
