# Universal Face grammar conformance

Face is Conduit's universal semantic waist for human encounter. It is
not a universal description of finished interfaces, and it does not promise
that an arbitrary existing interface can be encoded and reconstructed
identically.

The narrower claim is still demanding: truth intended for human encounter can
carry enough semantic and rhetorical structure that materially different Masks
can produce faithful, usable encounters without acquiring semantic authority.
This document is a pressure-test method for that claim. It is not a catalog of
standard interfaces and does not make their furniture canonical.

## Five boundaries

Classify every candidate fact before extending the grammar:

| Boundary | Owns |
| --- | --- |
| Plot and domain truth | what exists, remains true, and may happen |
| Projection | which of that truth matters in this encounter |
| Semantic composition | how the projected meanings rhetorically stand together |
| Mask realization | medium, interaction techniques, adaptation, and design |
| Show | the actual bounded encounter produced by that realization |

For example, two filesystem locations and their contents are domain truth;
that both locations matter now is projection; that they are being contrasted is
semantic composition; two side-by-side panes are one graphical Mask's
realization; and the particular rendered frame is a Show.

Do not move execution law into Face merely because it affects an
encounter. Choice, concurrency, synchronization, interruption, iteration, and
resume normally remain Plot truth. Face states their current humanly
relevant consequences: an action is available or refused, one subject follows
another, or a current activity has been interrupted. A proposal for richer
encounter structure must say why current actions, relationships, order, and
wording cannot express the meaning without importing application control.

## Admission questions

Before adding or changing a grammar construct, answer all of these:

1. What human meaning is lost without it?
2. Is that meaning authoritative domain truth, current projection, or rhetoric
   among projected truths?
3. Can a graphical, spoken, and deterministic-linear Mask preserve it without
   application-specific rewiring?
4. Can an unfamiliar Mask preserve the exact semantic identity and fall back
   honestly when it does not understand the domain?
5. Does the proposal name a medium, geometry, toolkit object, interaction
   technique, design system, or output artifact? If so, it belongs downstream.
6. Does it duplicate an existing subject, relationship, property, content,
   wording, action, input, disclosure, order, time, context, or composition
   statement? If so, collapse it rather than adding a synonym.
7. Does it prescribe application behavior across time rather than describe the
   present encounter? If so, it belongs in the Plot.
8. Is the value finite, exact, provenance-bearing, and safe to correlate back to
   the Face that authorized interaction?

Semantic composition is admitted sparingly. `contrast` and `emphasize` can
change what is communicated across media. `two-column`, `bold`, `window`, and
`spoken-first` prescribe realizations. The former may belong in Face;
the latter do not.

A rhetorical primitive is admissible only when removing it can change what a
human reasonably understands from the encounter across more than one medium.
That is stricter than proving that two renderers can implement it.

## Pressure-test corpus

Use the following archetypes to discover omissions, not to populate a closed
enum. For each specimen, record domain truth, projected truth, rhetorical
composition, Mask-only decisions, Plot-only interaction law, and what graphical,
spoken, and linear realizations must preserve.

| Archetype | Meaning that must survive | Tempting but non-universal furniture |
| --- | --- | --- |
| authored talk | progression, emphasis, reveal, comparison, supporting evidence | slide, title box, transition animation |
| program collection | programs, groups, names, availability, invocation | icon grid, overlapping windows |
| two-location file work | locations, membership, focus, selection, comparison, operations | left/right panes, function-key bar |
| spreadsheet | cells as domain subjects, formulas, dependencies, selection, edit actions | rows of pixels, column widths, formula bar |
| digital audio workstation | sources, tracks, temporal relationships, routing, current transport state | mixer strip, timeline pixels, fader |
| map | places, routes, containment, proximity when semantically asserted, current focus | tiles, zoom widget, screen coordinates |
| scientific plot | quantities, units, series, observations, uncertainty, comparison | axes in pixels, line color, legend box |
| book or article | sections, sequence, quotations, references, emphasis, subordinate matter | page size, columns, font metrics |
| command shell | current context, command intent, results, streams, terminal outcomes | prompt glyph, terminal escape sequence |
| nonvisual reader | the same names, structure, state, relationships, actions, and rhetoric | ARIA node, voice choice, braille cell |
| smart appliance | state, bounded values, commands, dependencies, refusal reasons | dial, touchscreen panel, voice phrase |
| collaborative classroom | participants, rooms, focus, activities, progress, authority differences | teacher dashboard, breakout-room tile |
| Patchbay | Plots, Gears, Ports, Cords, exact plan/play truth, source/destination roles | canvas, jack placement, zoom, cord curve |

The corpus is deliberately heterogeneous. Passing it does not prove
universality. A failure is useful: amend the grammar when a missing meaning is
genuinely cross-medium, collapse constructs when examples reveal duplication,
and leave a construct out when it merely reproduces a familiar interface.

For every specimen, compare semantic reconstruction rather than artifacts:

```text
authoritative specimen -> proposed Face -> several Masks
                                              -> what can the human understand?
                                              -> what can the human now do?
```

Ask those questions independently of each Show, then compare the answers. The
success condition is preservation of humanly relevant semantics, not visual or
verbal similarity. Same appearance does not establish same meaning; different
appearance does not establish different meaning.

The waist has two symmetric failure modes. Downward gluttony absorbs CSS,
panes, platform accessibility nodes, typography, and pixels. Upward gluttony
absorbs task models, state machines, scheduling, and application execution law.
Every conformance record must show that the proposed construct avoids both.

## Mask realization is planned design

Semantic preservation is necessary and insufficient for design quality. A Mask
may treat realization as an ordinary planned problem over the Face,
medium, available implementations and resources, context, policy, user needs,
conventions, and aesthetic knowledge. It need not be a fixed table from role to
widget.

That freedom grants no semantic authority. A Mask may select typography,
layout, speech structure, tactile grouping, interaction techniques, or a design
system; it may not invent facts, actions, relationships, availability, or
authority. Hand-authored and adaptive design remain possible downstream of the
waist so long as the resulting Show is correlated with the exact Face
and interactions return through its bounded semantic contract.

## Proof record

A grammar change should include at least one bounded specimen that failed
before the change and passes afterward. Record:

- the missing human meaning rather than the desired visual effect;
- why existing constructs could not express it;
- which boundaries own the new statement and its realization;
- graphical, spoken, and linear preservation evidence;
- finite bounds, identity, provenance, and stale-interaction behavior; and
- a negative test showing that nearby medium-specific machinery remains outside
  Face.

The grammar earns the word *universal* by surviving varied encounters while
remaining narrow, not by accumulating the nouns of every interface examined.

The current conformance fixtures and Rust crate still use `presentation` and
`Presentation` as migration-era serialized and implementation names. They
exercise Face; they do not establish a Presentation layer between Face and
Mask.

## Lifecycle conformance in development

Issue #5355 extends the existing reconstruction corpus with domain-derived
ConduitOS lifecycle specimens. The first executable specimen in
`semantics/presentation/tests/cross_mask_lifecycle.rs` starts from one actual
born/lulled `Body`, projects Overview and Library Faces at the same revision,
and checks their distinct context-bearing identities and common Body basis.
Production linear and deterministic aural projections preserve supplied names
and exact aural subject/action provenance. Neither projection changes the Body.

The interaction specimen uses the existing planned fixture Mask/Show helper to
check cross-context, stale-Show, invented-action and wrong-target refusal before
any domain effect. That is correlation-admission proof, not graphical execution.
The domain-owned Birth draft fixture also checks zero-Body linear/aural
projection and unavailable-choice refusal. The hosted owner continuity fixture
uses actual checkpoint write/read Plays to encounter three retained Todo items,
complete one, and reconstruct the verified lulled Face without a current Play.
It rejects the prior generation's digest while preserving the current receipt.
These are nonvisual projection and hosted kernel/Host Call proofs.

Native/browser prebirth and committed Todo Shows, audible speech,
Patchbay, Commons, unknown-domain fallback and pressure
remain required parts of the open four-Mask matrix. Separate reconstruction
accounts in `proof/conformance/presentation-waist` remain model proof and must
not substitute for these execution witnesses.

The Patchbay specimen in
`plots/patchbay/workbench/model/src/cross_mask_semantics_tests.rs` derives its
Face from the checked and expanded `plots/hello/main.conduit`, its exact graph,
and a Plan through the declared Text profile of the existing Host adapter. It requires
Gear, Port, and Cord subjects and verifies supplied names plus relationship
and action provenance through production linear and deterministic aural
readers. The complete shared `domain-derived/patchbay-face.json` is equality-
checked against that source projection. The checked browser Mask admits its
complete serialized input and preserves every supplied subject identity,
name, and role, then acknowledges its correlated Show through a simulated
DOM effect. This is source-derived projection and checked Mask mechanism
proof, not a live DOM/installed-owner journey or synthesized speech.

The open-domain contribution fixture in
`semantics/presentation/tests/face/open_domain_semantics.rs` contributes a
biology role and relationship through an ordinary Plot's exact Face
contribution basis. Linear and deterministic aural readers retain those open
IDs, their supplied names, typed action identity, and source provenance.
This is nonvisual reconstruction, not native/browser fallback execution.

The shared open-domain Face now also enters the production native scene and
checked browser Mask in focused tests. The native scene checks bounded,
fully visible primary/detail frames, inspectable open IDs, supplied controls,
and stale/unknown-control refusal without inventing a Gear diagram. The
browser Mask preserves open IDs in its view and emits the supplied typed action
with exact Face/Show correlation, rejecting stale and invented actions first.
Its acknowledgement is explicitly simulated. These checks prove native layout
and browser Mask mechanism behavior, not scanout, Chromium DOM interaction,
installed-owner effect, synthesized audio, or independent human observation.

`semantics/presentation/tests/face/finite_places.rs` adds an explicitly named
Commons-place fixture for #5355: three domain-owned open-role subjects and two
domain containment relationships enter the ordinary Foreground contribution
boundary of one Body Face. Linear and deterministic aural readers preserve
names, identities, and the supplied finite inspection actions. Containment
never manufactures connectivity or authority. The fixture's empty Plan and
lifecycle setup are test machinery; this does not implement or accept the
checked Commons application requested by #5359. Native/browser realizations
and the rest of this specimen's refusal matrix remain open.

The finite-place fixture now has a source-equality-checked shared Face record.
Native generic layout renders its names in bounded, fully visible frames and
retains containment in Details. The checked browser Mask preserves those facts,
rejects the Hub action when misbound to Workshop, and emits the exact targeted
inspection after simulated acknowledgement. These are fixture mechanism
witnesses; they are not the #5359 Commons product or live output acceptance.

The finite-place domain fixture explicitly withholds Crèche inspection with a
named unavailable reason. Its source-equality-checked Face carries that truth
into both Mask tests: native control resolution refuses it, and browser typed
return refuses unavailable agency and a retired Show before the valid Hub
inspection. Invalid returns preserve the supplied Face and do not consume the
valid action's sequence. This remains fixture admission/mechanism evidence.

The lifecycle target also checks a real wake transition on the same fixture
Body. Its born/lulled and awake Faces have distinct identities, share the Body
basis, and carry the actual Wake only when awake. Neither projection invents a
Plan or active Play. Both nonvisual readers retain exact Face revisions and
supplied action provenance while leaving current Body truth unchanged.

Source-equality-checked born/lulled and awake records now enter native layout
and checked browser Mask tests. Native primary/detail frames stay bounded and
fully visible; using the other state's Face or revision refuses. Browser
simulated-Show execution likewise refuses the other state's revision before
emitting one current supplied action. The shared Body is retained, and domain
Plan/Play absence remains distinct from each Mask's realization Plan/Play.

The zero-Body draft now supplies an equality-checked shared Face to native
layout tests. Primary/detail frames remain bounded and fully visible, retain
both available and unavailable choices, and refuse unavailable agency without
introducing a Body/Wake/Plan/Play. Browser Crèche currently carries the common
Birth draft through its application view; this is not proof of zero-Body
Face/Show correspondence, and no placeholder Body is permitted to fill the gap.

A focused zero-Body browser Fore test now runs the existing checked generic
Mask below the Body-owner carrier wrapper. It delivers the exact prebirth Face
to the DOM Host Call and returns the exact prepared Show through the admitted
Fore after simulated effect acknowledgement. The source keeps no Body, Wake,
domain Plan, or active Play; the Mask has its own realization Plan/Play.
This closes the fixture-level Fore correspondence gap without changing owner
membership/authority. Actual browser Crèche rendering and typed source return
through that route still require the canonical shell migration in #5356.

## Reference Face-to-Mask exchange

The canonical `face` Fore input is the complete validated Face, such as
[`open-domain-face.json`](../../proof/conformance/presentation-waist/domain-derived/open-domain-face.json).
It is not a layout, an application-state copy, or the partial summary below.
The checked Mask emits its correlated `show` Fore value; the `interaction`
Fore may return only actions and arguments supplied by that exact Face.

[`face-mask-reference.json`](../../proof/conformance/presentation-waist/domain-derived/face-mask-reference.json)
records the source/context basis, Host/Boot, Mask Plot/Plan/Play, Show, and actual
returned interaction for the open-domain browser fixture. The producing test
checks complete equality with this record. DOM acknowledgement is simulated;
no source effect, physical output, or human observation is claimed.

The request correlation excerpt is:

```json
{
  "face_id": "9f060a72c4991e1b5bb3080652bdde0ee8d093d5179dff11d6b73b300bbf6918",
  "face_revision": 30,
  "mask_plan_id": "15eb970ecf14f71deb0ad955d34ceb883776ccd143a47f03a365e6c008387751",
  "mask_play_id": "a39d4c2b9dfa654c0e01938f1aa3593ae4fe9db85676af22ce7de8a16ab777f2",
  "show_id": "show/c1d5e25552e0eab5e9e6e6f6f99856a01f475877c69a2fe3cbe360db6afac0ca"
}
```

The actual serialized typed return is:

```json
{
  "action_id": "lesson/answer",
  "arguments": [
    {
      "name": "lesson/answer-text",
      "value": [
        69,
        110,
        101,
        114,
        103,
        121
      ],
      "value_kind": "value/text"
    }
  ],
  "face_id": "9f060a72c4991e1b5bb3080652bdde0ee8d093d5179dff11d6b73b300bbf6918",
  "face_revision": 30,
  "identity": "1886da4a36530f551a9720cdb094cc647615a31ef34286b047bee27d871327e5",
  "sequence": 1,
  "show_id": "show/c1d5e25552e0eab5e9e6e6f6f99856a01f475877c69a2fe3cbe360db6afac0ca",
  "target": "concept/mitochondrion"
}
```

The Face ID/revision and Show ID must match the current acknowledged request.
The action, target, and exact byte arguments must match the Face contract.
Changing those correlations refuses before an interaction is emitted. Domain
Plan/Play basis is distinct from the Mask's realization Plan/Play.

The shared browser fixture exposed #5469: the view previously reported every
non-Available Show as Prepared, erasing Closed, Replaced, and Failed truth.
The view now maps all five existing lifecycle states explicitly. Focused tests
preserve distinct states, refuse argument pressure before emission, and refuse
returns after closure, replacement, or adapter loss without changing the Face.
This bounded projection correction is not a new lifecycle or renderer policy;
required hosted/stable acceptance remains open.

The shared Text-profile Patchbay Face also passes the native scene regression:
all subject names remain inspectable in primary/details, the exact graph is
recognized, every admitted document page is bounded and fully visible, and
supplied controls retain their action contracts. This is native layout proof,
not scanout or installed-owner effect evidence.

The checked browser Patchbay regression also compares every relationship
source, target, and kind and executes each available supplied action through
the checked interaction Fore with exact Face/Show correlation. Every stale,
invented, wrongly targeted, and unavailable proposal refuses before emission;
these tests apply no owner effects.

The browser lifecycle corpus regression now carries the source-verified
open-domain, finite-place, lulled, awake, and Patchbay Faces through checked
Mask execution. After simulated acknowledgement it closes, replaces, or
fails each Show (adapter unavailable). All fifteen cases refuse before
consuming the pending interaction Host request, expose no interaction egress
offer, and preserve the complete source Face. This establishes cancellation
and adapter-loss refusal at the checked Mask boundary; it does not establish
actual DOM cancellation, remote disconnect recovery, prebirth, or committed
Todo coverage for that matrix.

The owner checkpoint reconstruction test now feeds each freshly verified Todo
Face directly into the production native scene. A real three-item checkpoint
write/read and one admitted completion retain the same Body and exact terminal
Signs. Native primary pages retain pending items and the current remaining
count; Details retain all three items, including the completed item's source-
authored SelectedDetail disclosure. All visited frames are fully visible,
and native preparation/rendering leaves the source Face unchanged. The same
Face enters linear and deterministic aural readers. This is real checkpoint
execution plus native layout proof, not scanout or a native gesture. The
native dependency is confined to the product's development tests.

The committed Todo producer/consumer proof uses
`CONDUIT_CROSS_MASK_TODO_EXCHANGE` to export the freshly executed owner test's
before/after Faces and verified read receipts to an absolute scratch path.
The explicit browser consumer test
`checked_browser_mask_consumes_actual_committed_todo_exchange` reads that
exchange and is ignored in ordinary unit runs because it requires the producer.
The producer is `verified_three_item_todo_and_completion_preserve_nonvisual_meaning`
in the `conduit` binary test target. Run it first, then explicitly run the browser
consumer with the same exchange path. The record's Body and terminal Signs are
checked rather than normalized to deterministic fixture identities. The
fresh producer/consumer sequence is registered as
`check.committed-todo-mask-exchange` in the workspace gate, owned by the products
shard. Its runner allocates a private temporary exchange, executes both exact
test selectors, requires one passing test from each, and removes only its own
scratch directory. Registration and local runner success do not establish
hosted candidate/integration or stable acceptance.

The fresh committed Todo browser consumer now refuses stale Show, stale Face,
invented action, and unavailable lifecycle action proposals before emission.
For each exact before/after Face it emits the supplied task-2 completion
through the checked interaction Fore and verifies Face/revision, Show, action,
and target IDs. It deliberately does not submit this return to the owner;
the producer's admitted task-1 completion remains the sole committed mutation
in this proof, and the returned browser action is mechanism evidence.

The fresh committed Todo consumer now exercises both actual before/after Faces
against the declared add-text bound and three retired Show states. Oversized
text refuses with the Show still available. Closed, replaced, and adapter-
failed Shows refuse the otherwise available add action. Every refusal retains
the pending interaction request, exposes no return offer, and preserves the
complete committed Face. This closes the committed-Todo gap in the checked
browser cancellation/loss matrix, while live carrier/DOM recovery remains
separate acceptance work.

The zero-Body browser Fore test now completes the typed return boundary as well
as the Face/Show exchange. Prepared Show, stale Face, invented action/target,
unavailable Plot, over-limit name, and closed/replaced/adapter-failed Show
proposals refuse through the canonical Face interaction contract. The supplied
`creche.suggest` then traverses the actual checked Mask interaction Fore with
identical encoded bytes and exact decode equality. Face bytes and absent
Body/Wake/Plan/Play basis remain unchanged. Host effects are simulated, no Birth
is applied, and this does not implement the visible browser zero-Body shell.

The fresh Todo consumer also compares source-derived semantic reconstruction:
subject identities/names/roles, Status text, and relationship endpoints/kinds.
Its negative observations remove the essential progress subject, remove only
its text while preserving counts, invent a Port/Cord relation, or redirect an
existing endpoint to an invented Body/Port. Each fails the comparison for both
real committed states. This is a falsifiable semantic-loss oracle over the
actual checked browser projection, not runtime renderer attestation or a new
owner-side refusal mechanism; #5372 owns that further correspondence boundary.

### Parsec binding prerequisite audit

The required Parsec/remap specimen remains unimplemented. The existing
`semantics/human/src/input_chord.rs` maps structural modifier/key combinations
to core chords and explicitly separates them from text and product actions.
The bounded human interaction contracts validate proposals and application
outcomes. Native `native_face_scene/input.rs` resolves the current supplied
Face action through local focus and emits Show-bound typed interactions. None
of these audited contracts supplies the current action/control/label,
Host/Boot, mapping revision and availability witness required by #5369.

The smallest prerequisite is #5369's bounded binding witness and its checked
application/input Plot admission, reused by this corpus. That prerequisite
must precede claiming the Parsec part of #5355 complete, even though the fuller
prompt extension follows the main product path. A manually authored Fire map,
a generic Enter activation, or a string containing "Press Fire" cannot stand
in for current verified binding/remap truth. Keep raw input mechanics and
binding authority outside Face and Look; reuse existing chords/typed returns
rather than introducing a universal Button or Mask-local game reducer.

The first binding prerequisite now has a canonical Conduitese proposal carrier,
`ActionInputBindingProposalInfo`, generated from presentation `types.conduit`.
It reuses `GeneratedActionAffordance` for action identity and source revision,
and carries bounded Face/Show, input Plan, Host/Boot, control identity/label,
mapping revision and requested availability. It is outside Face and Look.
Construction or raw serde transport is not binding admission; checked native
decoding must enforce canonical bounds before a later policy gate uses it.
The carrier tests cover exact round-trip, source reference, empty/oversized
labels, zero revision, invalid version, and unknown layout/callback fields.
A checked input Plot producer and current-source/Host/Show binding admission
remain required. No truthful Fire prompt or remap completion is claimed yet.

Compiler/planner custody for the binding producer now reuses the configured
Mask gate's actual source expansion and selected Plan reproduction through
`prepare_checked_source_plan`. Its returned wrapper cannot be constructed from
copied IDs, and tests reject a substituted Plan and replaced Host Boot. The
configured Mask still performs its original Mask/Fore admission between those
two shared checks, preserving existing refusal order and source/recipe guards.
This is source/Plan correspondence only: it grants no resource reservation,
physical control possession, current binding, active input or Show authority.

`components/start-input-binding.conduit` now owns three typed proposal-producing
Plots: pregame Fire/Enter mappings to `game/start`, and a distinct gameplay Fire
mapping to `game/shoot`. Current context enters through a Fore rather than
startup configuration, avoiding a self-referential input Plan identity.
Checking includes the canonical native declarations and their laws; an imported
shape alone refuses dynamic construction. Evaluation of the actual expanded
pure-expression program proves that caller control labels/action names cannot
replace the source literals while Face/Show and mapping revision are retained.
This is source declaration/expansion/evaluation proof. It establishes neither
a selected physical input binding nor a gameplay state transition; producer
execution/Plan custody and current binding admission remain required.

Each binding policy now also places through the actual std pure-expression
capability definition in a named fixture Host advertisement. Ordinary planning
retains explicit bounded external Fore queues, selected Host/Boot, and the
shared source/Plan custody gate reproduces the complete Plan. Removing the
required capability refuses placement. The std-offer dependency is development-
only. This extends the proof to producer planning; it is still not installed
Host execution, physical control availability or current binding admission.
