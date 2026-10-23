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
