# Checked mechanical layout preparation

Owning issue: [#6159](https://github.com/dancxjo/conduit/issues/6159).
Consumers: #5994, #6038 and the Crèche reference #6174.

This slice checks finite nested composition below Face. It does not change
`component-mask@2`, select a renderer, measure text, arrange pixels or improve
the current Crèche screenshot. Those are remaining delivery obligations.

## Custody and bounds

`component-layout.conduit` uses the existing authored Type and generated native
binding machinery. The profile names an existing recipe identity; preparation
runs `prepare_component_recipe` against an immutably borrowed exact Face before
checking composition. Existing property/action binding checks remain authoritative.
Visual child order does not establish semantic or action traversal order.

There are at most two ordered candidate trees, 32 nodes in total, depth four,
four tracks per logical axis and spans of one or two tracks. Each candidate
has its own node array and root. Every node must be reachable exactly once;
cycles, duplicate parents, invalid references and overlapping grid cells refuse.
Every recipe region occurs exactly once per candidate, accompanied by one
explicit generic fallback leaf for unmatched valid subjects. Empty groups are
legal but cannot replace those leaves.

Logical alignment, keep/accessible-continuation policy, bounded grow/shrink,
and spacing in sixteenths of the selected root text em describe composition.
Spacing has no physical meaning until the selected Mask resolves actual Look
and Host text metrics. Face facts and authority remain unchanged.

## Remaining implementation

A versioned selected Back must explicitly admit this profile before product use.
It must measure actual selected text and accessibility metrics bottom-up,
arrange top-down, choose candidates by measured feasibility, report resource
and geometry shortages distinctly, and produce newly admitted Show geometry
and hit regions when Face, Look, metrics or viewport changes. Current scene,
text, row, surface and Application limits remain unchanged.

The contract tests use nested unrelated composition and a bound label/field
grid retaining a Face association. They establish preparation laws only;
real Crèche and unrelated product compositions, measurement/arrangement,
browser/native correspondence and normal-product QEMU proof remain open.

## Local validation

Six new preparation tests, seven existing recipe tests and two existing typed
action-binding tests pass. The positive nested composition reaches the admitted
depth-four boundary through column, row, grid and leaf. Owning library Clippy
passes with warnings denied. The first compile exposed generated-binding API
mismatches; the corrected implementation uses the existing generated variant
records and explicit serde record selection. No generator behavior was changed.

Commands used the stable toolchain:

```sh
cargo +stable test -p conduit-face --test component_layout --test component_recipe --test component_action_contract
cargo +stable clippy -p conduit-face --lib -- -D warnings
```

Compressed logs are retained in `evidence/checked-layout-profile-20261023/`.
No renderer, emulator, browser, CI or accepted-release result is inferred.

## Selected native text metrics prerequisite

The native font generator now retains the actual rasterization em size beside
ascent and line height. `intrinsic_text_metrics` uses the existing selected ink
and base/mark cluster rules to report the minimum indivisible inline width,
the longest explicit unwrapped line, selected line height and explicit line
count. Maximum usable extent remains the separately admitted container bound.
The API accepts the existing 192-byte payload and 100–200 percent scale, performs
two bounded scans and allocates nothing. It does not add general shaping.

The production `TextLayout` cursor consumes this same minimum in its existing
pre-paint width refusal. Portable em-sixteenth spacing can resolve against the
selected font size with one outward rounding step, rather than using line
spacing as an em substitute. Profile-driven spacing is not yet installed.

Twenty-five owning typography tests pass, including comparison against actual
cursor ink and height at minimum/preferred widths, both typography selections,
100/125/200 percent scaling, negative bearings, combining marks, explicit lines,
tabs and unsupported-scalar fallback. Existing raster confinement, repeatability
and shared measurement/paint checks also pass. Strict native owning-package
all-target Clippy with `native-compositor` and warnings denied passes.
Compressed `native-intrinsic-tests.log.gz` and `native-intrinsic-clippy.log.gz`
retain those results. This is CPU evidence, not a new
QEMU or browser run. The remaining native consumer must still admit the new
profile explicitly and implement bounded measurement, arrangement and continuation.

## Versioned admission and source custody

`layout_component_mask_kind` retains the ordinary component Kind identity with
explicit contract revision `conduit.face/component-mask@3` and a required typed
layout startup value. The version-two constructor and sources remain unchanged.
Selected configuration retains an optional layout; absent layout is omitted
from serialized version-two records, as checked by compatibility tests.

Preparation authenticates the selected renderer's declaration, Type and concrete
configuration. Recipe and layout must both originate as literals in the same
checked component invocation, then pass unchanged through the exact authenticated
Back expansion path. Sibling literals, outer substitutions, equal-byte parameter
substitutions, altered expression forwarding and missing required layout refuse.
This is custody of configuration, not a renderer acknowledgement.

Twenty-four focused Face tests pass, including six new custody cases. The
native offer package's eight tests pass, including the unchanged selected
version-two paths, both existing storage profiles, and version-three planning
with the reviewed Crèche and unrelated Todo sources. An old native offer cannot
be relabelled to satisfy the distinct layout renderer contract.

The new native selected-Back fixture is explicitly `cfg(test)`: ordinary Host
advertisements do not offer it and production does not select it yet. This
prevents preparation proof from advertising an unfinished renderer. Real-source
single-column and wide/narrow alternatives preserve the existing region bindings;
their native measurement, geometry, continuation and input proof remain required.
Current authored invocation syntax uses one physical line; an initial multiline
literal was rejected and corrected without changing the parser.

Configuration preparation moved unchanged out of the native Mask facade into
`native_face_mask/configured.rs` before adding a consumer. Twenty-one existing
native Mask tests and strict native all-target Clippy pass. A first native
compile found an older generated Face output in the shared cross-worktree cache;
forcing the current build script to regenerate its output corrected that local
cache state. The failed diagnostics and positive results are retained separately.
