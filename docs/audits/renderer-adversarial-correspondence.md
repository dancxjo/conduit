# Adversarial renderer correspondence

Issue #5644 extends the accepted #5643 first-add producer. It retains one real
installed Todo owner, the original pre-Add Show, and the current acknowledged
Show after exactly one ordinary browser Add. The generic renderer supplies
native buttons, labeled inputs and disclosure summaries from the exact Face
subject/action names. Its local `data-owner-action` keys only locate controls;
SDK acknowledgement and the existing WASM/owner gate retain authority.

The missing product seam was acknowledgement of a prepared DOM realization
without checking that its offered actions had meaningful native controls. The
Handbook now checks its generic renderer before acknowledging the prepared
Show. Each available action must have a native named keyboard control and
labeled reachable inputs, or rendering refuses with
`RendererActionInaccessible`. A closed native disclosure supplies its named
keyboard path. Hidden, inert, transparent, clipped, missing or nameless controls
cannot silently acknowledge a usable Show. This conservative check certifies
this renderer's native control structure, not arbitrary CSS or a general
accessibility engine. It introduces no semantic action registry or Body state.

| Retained installed case | Expected boundary | Owner effect |
| --- | --- | --- |
| Ordinary current Add | SDK, WASM, owner Plan/Play | One retained accepted mutation |
| Invented action / wrong target | `UnknownAction` / `WrongTarget` | Whole owner snapshot unchanged |
| Missing / unknown / duplicate argument | Distinct typed SDK refusal | Whole owner snapshot unchanged |
| Wrong kind / oversized / malformed Unicode | Distinct typed SDK refusal | Whole owner snapshot unchanged |
| Actual pre-Add Show after the Add | `StaleShow` at SDK | Whole owner snapshot unchanged |
| Focus, Tab and scroll | Mask-local movement | Whole owner snapshot unchanged |
| Hidden generic surface during refresh | Rendering refusal before acknowledgement | Whole owner snapshot unchanged |
| Restore generic surface | Named reachable current control | No rejected-event replay |

Every rejected trusted DOM event records its exact adapter code, gate, Show,
Face and unchanged canonical owner Face. Adapter refusal precedes WASM dispatch;
these cases do not claim to exercise Rust core refusal variants. SDK codes use
that existing vocabulary without replacing owner admission. The independent
Face `human_interaction` suite covers typed constraints, revision/context,
duplicate delivery, finite queue pressure, cancellation and failed surfaces.
The renderer-only Chromium fixture tests pseudo-control, nameless control,
hiding, opacity, inertness and clipping before acknowledgement; its callback
is explicitly not an owner effect receipt.

The supported installed entrance remains `cargo xtask prove todo-journey` with
`--fresh-body-source`, `--conduit-bin`, `--handbook-package`,
`--pinned-playwright`, `--state-dir` and `--output`. Packages and receipts must
identify one clean exact Source. Full browser validation enters through
`cargo xtask check browser`; focused raw runner calls are internal diagnostics.

Validation receipts for this change are pending. This document makes no claim
of passing packaged Chromium, native graphical, QEMU, speech-provider, physical
or human proof. Native wiring and installed QEMU journey remain #5604/#5605.
Detailed Look/accessibility conformance belongs to #5367/#5630–#5632 and
#5379/#5656; this check preserves meaningful agency or refuses this rendering.
