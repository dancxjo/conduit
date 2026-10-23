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


## First native row/column consumer (preparation only)

The native preparation path now consumes the checked layout using selected ink
metrics, bottom-up demands and top-down inline allocation. Both authored
candidates are measured; lexicographic page count and total fragment block
extent select the result, with authored order breaking ties. There is no fixed
viewport breakpoint. Weights are named `inline_grow` and `inline_shrink` to make
their axis explicit; they do not claim CSS main-axis or block flex behavior.
Grid and unsupported shrink weights refuse this consumer and retain the complete
generic fallback. Version-two configuration remains on its existing path.

Accessible continuations take the largest safe prefix fitting remaining height,
row and command budgets. Declared Keep groups and associated field clusters
that fit a fresh page remain indivisible. Continuations retain inherited group
padding. Each split scans at most twenty rows. Original semantic item/chunk
indices determine traversal; geometry does not reorder controls. Limits remain
48 commands, 192 text bytes and twenty rows per page.

A real-domain CPU conformance case compiles the reviewed Crèche and Todo sources
and retains their complete content, control identities, disjoint rows, actual
frame hit targets and original Face at 1280×800, 640×480 and 320×480. Wide cases
must actually use two columns; narrow Crèche must collapse coherently. The first
wide Crèche assertion exposed coarse fragment packing: wide measured two pages
and 1112 block units, losing to narrow at two pages and 1040. Safe-prefix packing
reduced wide to two pages and 852 without a domain-specific rule. At 320 pixels,
wide needs five pages versus four for narrow. A separate initial Todo assertion
used the absent TextEntry region; correcting it to the real Collection region
retains the two-column requirement. Both negative records are preserved.

Ten mechanical tests, the real-domain six-extent case and strict all-target
Clippy for native, Face and native offers pass. These are preparation/CPU results.
The ordinary Host offer remains test-fenced. Candidate storage is bounded by
MAX_ROWS, but the simultaneous current/best candidate peak has not yet been
measured and admitted in the freestanding application grant. No normal-runtime,
QEMU, browser or complete #6159 acceptance is claimed. Grid, block flexibility,
selected runtime admission and required platform proof remain open.

All twenty-one existing native Mask tests also pass after installing the optional
layout dispatch; their log is retained as `native-layout-dispatch-tests.log.gz`.


## Resource and explicit-selection follow-up

The structured scene now validates the exact Face and extent and prepares its
selected geometry directly, rather than first retaining a complete generic
scene and then replacing it. The version-two and fallback constructors retain
their existing behavior. Candidate evaluation retains only scalar scores, drops
each candidate's rows, and prepares the winner once. With at most two candidates
this admits at most three fixed arrangement passes and avoids simultaneous
current/best document storage. This is a storage reduction, not yet a measured
freestanding grant receipt.

The explicit layout offer/preparation functions now compile as ordinary library
APIs so callers can prepare an actually matching selected Back. This supersedes
the earlier test-only export fence. Ordinary Host advertisements still do not
offer this Back; old offers cannot satisfy it, and no installed default selects
it. A caller must supply the exact matching offer before planning. This does
not turn the availability of native consumer code into authority or a Show.


## Bounded grid preparation and remaining first-page pressure

The native consumer now negotiates up to four explicit inline/block tracks and
one/two-track spans. Single-track intrinsic contributions precede spans;
deficits distribute deterministically in track order. Assigned inline widths
feed actual height-for-width measurement. Empty tracks have zero intrinsic
size but retain all declared gaps. Span-connected block bands stay coherent;
an oversized cell/band explicitly refuses to the complete generic reader.
There is no implicit cell placement, convergence loop or domain priority rule.

An independent account-form Face uses real typed bound slots and label/field
relations, unavailable input, fallback meaning, both span axes and shared tracks.
Eighteen mechanical tests pass, including zero/one/many cells, minimum conflict,
empty/leading/skipped tracks, band continuation, row/command pressure, aggregate
text admission, complete content, source order and actual frame hit targets.
Review caught dropped empty-block-track gaps (44 instead of52); the negative
and corrected regression are both retained. A separate initial unsupported-shrink
fixture omitted a required region and was corrected without changing the law.
The original general Crèche/Todo six-extent case and the exact selected
configuration case pass, as does combined strict owning-package Clippy.
The configuration case preserves every action/argument control versus the
established generic baseline and every fallback text command across primary
and Details continuation. Ordinary Host selection remains unchanged.

A stronger separate acceptance check remains deliberately failing at this
checkpoint: all four checked starting Plots must be visible on wide Crèche's
first page. Current full material consumes48 commands after two checked Plots;
including the other two would require56. Geometry fits, but commands do not.
The held branch has no PR and does not claim full-package green or #5608
acceptance. The failure is retained without an ignore or a relaxed limit.
A following bounded Mask decoration change must reduce actual command cost or
an explicit authored composition must defer material; geometry cannot invent
source priorities or drop meaning. Freestanding resource admission and normal
QEMU proof remain required. Block-axis flexibility is still outside this slice.


## Compact Boolean decoration removes the first-page command conflict

The retained56-versus48 failure is now corrected without changing source text,
controls, command limits or layout priorities. A source-authorized Boolean
choice uses the already-admitted Confirm checkmark for true or an outlined square
for false, one graphics command each. It does not add another full button outline.
A separate inset row outline still marks keyboard/pointer focus without relying
on color. Generic actions retain their ordinary button decoration. No graphical
primitive, icon vocabulary, font glyph, semantic Kind or local selection truth is
added. The existing exact Boolean association and two-value contract remain the
only admission for this choice realization.

Measurement reserves the same command cost actually emitted. Four checked
choices save eight commands. The stronger real wide Crèche test now passes,
including focusing each of the four checked controls on page zero and assembling
the focused frames within48 commands. The complete original Face, text and exact
controls remain unchanged. Wide/narrow alternatives still use measured scores.

The focused scene suite passes135 unique checks, with its existing opt-in Node
cross-Mask case skipped; Cargo also ran the mirrored binary test target. Twenty-two
native Mask checks pass. The dedicated four-choice focused case passes. Existing
state/Show refusal, Unicode, scale, wrap, raster confinement, command/row pressure,
traversal and hit checks remain active. Initial fixture failures expected obsolete
checkbox costs/full-row boxes; pressure fixtures now add a real existing icon to
reach their intended pressure, and frame checks verify the actual source-owned
indicator, focus geometry and unchanged hit binding. Both negative and positive
logs are retained. This is CPU preparation/raster evidence, not a selected ordinary
native runtime or new QEMU capture. Native grant admission and platform proof remain
open before #5608/#6159 completion.


Current dev synchronization includes merged #6278 at `3f801610e`. The merge
was clean. All24 native Mask checks pass afterward, including actual stale
submission regressions, and strict all-target Clippy for native, Face and native
offers passes. The complete native Face-scene subtree is byte-identical to
`4dbc34893`, retaining the135 passing scene checks and focused four-choice result.
The final retained logs are `post-pressure-merge-mask-tests.log.gz` and
`post-pressure-merge-clippy.log.gz`. No new CI campaign was queued for this held
preparation branch. Normal-runtime selection and required QEMU admission/proof
are the next remaining implementation boundary.

## Explicit normal native deployment (in preparation)

The normal product now explicitly offers the @3 layout Back and the existing
configured wrappers select it only from that exact observed offer. Legacy
advertisement APIs remain @2. A malformed advertised new Back refuses rather
than silently downgrading. The actual Mask Play dispatches the selected new
renderer through its existing finite renderer realization. CPU evidence: 24
actual native Mask tests, 11 Mask Play tests, 10 deployment offer tests, 10
collector tests and four boot collector tests pass. These are not a new QEMU
run. Current collector receipts require exact new implementation/capability
identities and both recipe and layout canonical fingerprints; old or mixed
renderer tuples refuse, and Look changes cannot substitute layout structure.

### Join prerequisite #6288

The first real-product join tests caught `JoinError::Bound`. Replacing the old
configured Back with the new one (rather than advertising both) leaves the
representative 64-byte Host/Boot advertisement at 32,570 bytes. Its complete
serial observation is 33,310 bytes and routed request is 33,205 bytes. Neither
fits the former 32 KiB envelope. Both negative runs and the size diagnostic
are retained beside the positive checks. No configuration or semantic contract
is omitted, no fabricated smaller default is installed, and no fallback
renderer is removed to make the packet fit.

#6288 explicitly admits a 48 KiB serial envelope and WebSocket frame, with the
maximum binary payload excluding the same 14-byte framing overhead. The pure
protected Network codec and local codec share the same ceiling, checked by a
compile-time equality assertion. Fixed local receive/transmit arrays grow by
16 KiB each; protected framer receive/transmit, State input/output and Root
frame/payload auxiliary storage grow by 16 KiB per corresponding buffer. The
protected 16 KiB exchange window remains unchanged and carries chunks. The
Network heap grant and Application private grant remain unchanged. Actual
native admission remains required; CPU bounds checks do not prove memory
headroom. Semantic scene limits remain 48 commands, 192-byte text, 20 rows and
eight retained surfaces. Exact-product serial/routed tests now pass, retain
non-membership/no-Plan/no-Play facts, test longer identities and refuse oversized
requests. Four fixed-storage WebSocket checks and final strict all-target Clippy
for native, Face, native offers and Network core pass. Required QEMU remains
pending.

## First normal-product QEMU run: c65a9d4fa

The canonical product-host ISO built successfully from clean published
`c65a9d4fa152f028eb1691524cab00ae2a112c3e`, SHA256
`966f5c4b331c291e21d8db257dbbd61ff9852271d7f3196979b6977824270914`.
Its 1280×800 control proof passes: 223 focus observations, 15 original captures,
47/48 command high water and 192-byte maximum command payload. All four actual
source Plot controls have first-page hit regions, including after the long
Unicode edit. The actual capture shows Body/naming and search/Plots in two
coherent columns. Exact component, recipe and layout fingerprints stay invariant
across the two real Look selections; Shows and input remain source-bound.

The 640×480 control proof also passes interaction conformance (249 focus
observations, 35/48 command high water). **Visual inspection rejects this as the
intended narrow-layout acceptance:** it still compresses two columns. Its
original screenshot and successful behavioral receipt are retained together in
`c65-narrow-compressed-columns`; success does not establish coherent collapse.

The next normal graphical journey reaches the actual Patchbay checked Source
path, then fails `proof-viewport-mismatch`: its largest complete native Face
record is 110,393 bytes versus the viewport collector's old 96 KiB cap. There
are only 38 Face and 20 focus records, in 2,057,321 serial bytes; this is not a
record-count or product scene limit. Retain the original partial journey and
failure in `c65-journey-viewport-bound`. No complete journey or Source traversal
claim is made for this run. The corrected viewport collector uses the existing
journey correspondence 2 MiB Face-record cap, retains the 96 KiB focus cap,
256-record cap and 16 MiB serial cap, and preserves exact Show custody and
malformed/overbound/truncated-line negatives.

### Measured minimum instead of a viewport breakpoint

Disabling row shrink outright failed the strong wide Unicode/four-choice test;
that negative is retained. The missing composition fact is an authored usable
panel minimum, distinct from each text cluster's hard-wrap minimum or full
unwrapped preferred demand. The checked Group now carries a finite outer inline
minimum, including padding, in sixteenths of selected root text em (0..4096,
zero adds no extra minimum). Crèche and Todo wide panels request 20 em each;
the selected font and accessibility scale resolve that request. Bottom-up
measurement propagates these minima through existing row/grid allocation. The
explicit single-column alternative wins when they cannot fit; no screen-width
cutoff or domain-label lookup is added. Candidate/node/depth/track/work-pass and
scene limits remain unchanged. The em resolver covers the new 256-em maximum;
spacing and padding retain their own existing 16-em Type bounds.

Strengthened source checks require coherent collapse at 640 as well as 320,
while retaining the strong wide four-choice/Unicode result. A separate native
minimum test verifies font-scale-dependent feasibility on unrelated fixture
content. Portable bounds/roundtrip, native Mask, intrinsic metrics and viewport
collector checks accompany the change. New-source QEMU remains pending until
these checks finish and the source is committed.

During this build, an inventory and `fuser` check found 1,121 unopened,
regenerable cross-target `.rlib`/`.rmeta` files in inactive caches. Removing only
those recovered 2,810,023,868 bytes (about 2.6 GiB). Executables, generated values,
images, source, unique evidence, active x86 outputs and active build artifacts
were preserved. Earlier separate browser-cache cleanup is recorded independently.

The strengthened CPU checks now pass: two strong real-source geometry/content
checks, 19 mechanical checks (including selected-font minimum scaling), all24
native Mask checks with 640×480 added to actual selected-Back scanout, three
intrinsic metric checks, seven portable layout checks and six authenticated
layout-configuration checks, ten offer checks, three actual-product join checks,
three viewport checks in the owning xtask target, and strict all-target Clippy
for native/Face/offers/Network. The new exact join sizes are 33,903 serial and
33,798 routed bytes, within #6288's admitted finite envelope. An initial
viewport test invocation selected zero tests in the host-make facade; the
corrected owning xtask invocation ran the three tests reported here. It is not
counted as prior proof. All final logs are retained under `minimum-final-*`.

## Corrected current normal-product proof: 80260a13f

The minimum-sizing correction and viewport guard are synchronized through dev
`e57706b15` in clean published Source
`80260a13fbc7f611f8f7dda7804d195adfa6b2ec`. Its actual product-host ISO SHA256 is
`5912de28096e4a236ba94b3b7cd01e1e4654f65732b0f6b9203f620ba6a1e4e2`.
[Current original packets](evidence/checked-layout-profile-20261023/802-README.md)
retain exact source/image identities, immutable input observations, original
PNGs, paired receipts and losslessly compressed full transcripts.

| Current case | Observed result |
| --- | --- |
| Wide controls,1280×800 | Two coherent panels; all four actual Plot controls on page one, also after the long Unicode edit; 223 focus observations,15 captures,47/48 commands |
| Narrow controls,640×480 | Genuine single-column collapse with explicit continuation,253 focus observations,17 captures,25/48 commands |
| Search,640×480 | Five real choices/four selected; exactly match,no-match,clear commits, no selection mutation, full inventory restoration and keyboard cycles;108 focus observations,11 captures,25/48 commands |
| Normal graphical journey | Birth/Home/Body/Patchbay complete on the same actual Body/Plan/Play;49 checked Source subjects,14 Cords/two boundary bindings, continuation and typed return;27 ordinary+20 refusal/Details+seven Source captures |
| Graphics provider absent | Same uninstrumented ISO, genuinely no VGA device; typed `display-absent` terminal refusal, exit35, zero acknowledged Shows/scanout frames/source changes |

Both current control runs retain real two-Look selection and unchanged checked
component/recipe/layout identity, forward/reverse traversal, naming-field pointer
focus, checked Unicode draft edits and incomplete-argument refusal. The separate
normal journey records source-owned Boolean4→3→4 through two keyboard and two
pointer actions, with no duplicate held-motion effect. Every command remains at
or below192 UTF-8 bytes.
Original wide and narrow captures were visually inspected; the narrow c65
compressed-column negative remains separate. Current Home visibly shows Commons
/ Hub with its actual four resident choices, rather than the stale canvas image.
The current Source diagram and its explicit five-of-sixteen shown/eleven omitted
status were inspected; complete relationships remain accessible through all49
Details subjects and continuation, not simultaneous visibility of every edge.

The complete supported graphical-profile command exits zero. Its additional
four headless product images exclude graphical assets as required by the
existing entrance; those are build/asset checks, not cross-architecture GUI
execution. All five independent current cases use the same native product ISO.
Normal Source preparation completes within the unchanged Application private
grant. This is observed admission for this product path, not a universal
worst-case heap theorem or new hostile-code confinement claim.

The first802 normal attempt failed a deterministic local evidence-directory
precondition: the earlier Source-capture directory still existed. Its packet is
retained in `802-output-collision`. After preserving and moving those prior
bytes, a fresh-output run completes. No product timing, action poll, forced
interaction, retry budget or source code changed between attempts. The failure
is not hidden or counted as successful product proof.

Independent original-byte verification covers97 current captures:17 narrow,
15 wide,54 ordinary/refusal/Source journey and11 search PNGs. All declared byte
lengths, file SHA256, every PNG chunk CRC, dimensions and independently decoded
RGBA SHA256 match. Only actual-run manifest files are included; stale unreferenced
working files are excluded. `802-packet-inventory.json` binds every archived
original length/hash, including losslessly gzipped receipts; the verifier and
its report are retained. Missing optional Pillow was handled by reusing the
existing repository stdlib decoder, without a new dependency. The recorded
97 images are verified bytes, not regenerated screenshots.

### Scoped acceptance and resource ledger

The earlier accepted pressure and stale/copy-loss packets retain their exact
Sources and QEMU-appliance scope. They exercise unchanged production scene,
compositor and submission contracts; they are not relabelled as this normal
product run. Together with the current ordinary product cases, these supply the
#5608 layout/failure matrix before current merge/check acceptance. Search is
rerun because geometry/traversal changed. No physical hotplug is inferred or
required. Earlier unreproduced startup refusal evidence remains historical;
these results do not claim repeated-run reliability.

| Bound / observation | Disposition |
| --- | --- |
| Scene48commands;192-byte command text;20rows/page;eight surfaces | Unchanged; current prepared command high water47; earlier actual QEMU pressure proves atomic49th/193rd/ninth refusal |
| Two candidates;32total nodes;depth4;four grid tracks;spans1..2 | Unchanged; score/drop candidates then arrange one winner, at most three arrangements |
| Outer minimum | New explicit0..4096 sixteenths of selected root text em; Crèche/Todo panels20em; no viewport cutoff |
| Standard48 KiB / Collection256 KiB value profiles | Existing exact selected profiles retained |
| Application private heap2,072,576bytes;Network private heap1,028,096bytes | Unchanged grants; ordinary current journey/Source admission observed; maximum Network-frame local CPU tests remain distinct from live routed exchange |
| Native join / WebSocket frame48 KiB | Explicit #6288 finite increase from32 KiB; exact current signed serial33,903/routed33,798bytes; oversized refusal and Root/protected-codec equality retained; existing hosted owner admission limit is2 MiB |
| Runtime arena/ceiling268,435,456bytes;glyph corpus/fonts | Unchanged; earlier Source-pinned pressure and typography evidence retained |
| Performance | Finite pass/storage/command ledger only; no calibrated boot-speed or latency claim |

#5519 and #5358 are closed against accepted/published3f801610e after full
Integration39947738203 and actual49-file/seven-Source-PNG custody verification
in #6276. #5994 step15 is reconciled. Step12/#5608 still requires this new
implementation's reviewed merge and relevant current checks; older CI is not
new-source CI. #5994 closeout remains open until that delivery completes.

### Broader #6159 remains open

Independent read-only review found no blocker in the new outer-minimum law:
selected-em resolution has one outward rounding, padding is included once,
preferred demand is raised to the minimum, and actual allocated widths are
rechecked. This is a first ordinary native consumer, not complete #6159.
Remaining scope includes authored maximums/general block flex, baseline
alignment, richer grid-band continuation, browser@3 realization/proof, further
invalidation/typography coverage and resource-disposition detail. Preferred
measurement currently takes bounded192-byte chunks rather than the longest
whole unwrapped item line across chunk boundaries. Full shaping/bidi/CJK and
physical/human evidence remain outside these claims. Unsupported constraints
retain explicit refusal/generic fallback; they are not silently called supported.

Workspace Rust formatting and patch hygiene pass after the evidence closeout.
The measured product Source remains80260a13f; later documentary commits do not
relabel its actual pixels, binary or source identities.
