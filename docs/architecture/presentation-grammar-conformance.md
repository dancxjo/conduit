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
The view now maps all five existing lifecycle states explicitly. Its serialized
`show_state` contract is Prepared → `prepared`, Available → `available`,
Closed → `closed`, Replaced → `replaced`, and Failed → `failed`.
`show_failure` retains the authoritative failure enum (`AdapterUnavailable`,
`OutputRejected`, or `DeliveryFailed`) and is null when no failure exists.
Only an Available Show can report admitted interactions. A serialization
regression checks the five labels, failure detail, and exact Face identity and
revision; the former non-Available-to-`prepared` projection fails this corpus.
Focused tests
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

### Parsec binding prerequisite

The Parsec/remap specimen remains incomplete. Existing human chords separate
structural modifier/key combinations from text and product actions; native
focus/activation returns the supplied Show-bound Face action. Neither provides
current control/label, Host/Boot, mapping revision and availability authority.
The #5369 binding prerequisite must be admitted before claiming this portion
of #5355 complete. A manual Fire map, generic Enter activation or prompt string
cannot establish current binding truth. Binding authority remains outside Face
and Look; there is no universal Button or Mask-local game reducer.

The canonical `ActionInputBindingProposalInfo` carrier reuses
`GeneratedActionAffordance` and bounds Face/Show, input Plan, Host/Boot, control
identity/label, mapping revision and requested availability. Checked native
decoding enforces its laws; raw serde is not admission. Carrier tests cover
round-trip, source references, empty/oversized labels, zero revision, invalid
version and unknown layout/callback fields.

`prepare_checked_source_plan` reproduces the actual checked source expansion
and selected Plan. Tests reject substituted Plans and replaced Host Boots.
Configured Mask admission retains its original source-check, Mask/Fore-check,
Plan-check order. This custody gate grants no reservation, physical control,
current binding, active input or Show authority.

`components/start-input-binding.conduit` owns three exact policy producers:
pregame Fire/Enter to Start and gameplay Fire to Shoot. Context enters through
a typed Fore, avoiding a self-referential Plan startup identity. Actual std
pure-expression offers place them in a named fixture Host with bounded queues;
missing offers refuse. Canonical native laws are required to construct outputs;
imported shape alone refuses. `produce_checked_input_binding_proposal` accepts
only reproduced source/Plan custody and revalidated context, evaluates the
single exact pure producer and retains its Plot/Plan provenance. It proves
source evaluation, not an installed Host receipt or control observation.

Face correlation resolves each produced proposal against the exact validated
current Face and available Show and derives its target from the supplied
action. Tests reject stale Face/Show/revision, absent/unavailable actions and
Closed/Replaced/Failed Shows. This does not authorize controller remapping.

`components/parsec-state-step.conduit` owns a finite transition: Start enters
gameplay; Shoot refuses before gameplay; stale Start and invented commands
refuse without changing state. `components/parsec-input-policy.conduit` selects
Start/Shoot from phase and Fire/Enter from the explicit remap choice. Tests
evaluate both actual checked programs, carry the Start transition output into
the policy, cover both phases/mappings/availability values, and retain exact
Host/Boot/input Plan, mapping revision and Face/Show/action revision context.
The caller supplies prior state and context. Retained Body state, installed
execution, current control admission and truthful native/browser/linear/spoken
prompts remain required; no physical input or delivered audio is claimed.

The zero-Body checked browser Mask Fore now has explicit pressure/cancellation
coverage. Filling its actual planned input queue yields `Full` at the retained
sequence; repeating that submission stays pressured. Cancellation before DOM
completion rejects a late Host result and revokes both Show and interaction
egress. A held typed prebirth return retains its exact sequence/value until
cancellation revokes delivery. The canonical Face remains unchanged and has no
Body or active source Play. These are scheduler/checked-Fore mechanisms with
simulated Host effects, not DOM rendering or a performed Birth.

Actual scheduler cancellation is now included alongside terminal Show loss for
all five shared Body-backed browser specimens. It refuses before interaction
emission, retains the pending request and unchanged Face, and revokes egress.
A repeated-input regression exposed a value leak after the interaction Fore
was already consumed: terminal refusal previously allocated before checking
request ownership, eventually reporting item-capacity pressure. The browser
return path now checks pending ownership before allocation, consumes it only
after successful completion, and releases a stored value when completion
refuses. Storage-pressure refusal preserves the request; draining storage lets
the same supplied action complete once, and repeated terminal proposals remain
terminal without exhausting storage. These are checked browser mechanisms;
installed owner effects and real DOM/cross-medium recovery remain separate.

The installed keyboard Host lifetime is now hardened before binding admission:
a duplicate request refuses without replacing the original pending request,
and cancellation or terminal input failure closes the host object against late
acceptance. Missing adapters and malformed inputs do not admit a request. The
new ownership/lifetime tests fail on the prior code and pass with the repair;
existing installed keyboard cancellation/reuse and continuous button-indicator
execution also pass. These mechanisms do not yet construct a current binding
witness or retained Parsec state.

The explicit `retained-dom-completion` browser journey scenario preserves the
sole retained Todo Body, reads its actual current state, and uses only supplied
DOM reopen actions to reach three pending items before a DOM completion. It
checks fresh acknowledged Shows, source-correlated verified write/read receipts,
observed accessible lists/progress, unchanged Look, and stale-return refusal.
The scenario requires exact-source owner/runtime/UI artifacts. It is prepared
and syntax-checked; current-source live execution remains unproved.

Private installed input-origin admission now retains the exact selected
placement and lowered call, current Host/Boot/offer generation, and issued
fragment Play. Body preparation separately retains a prospective Body Play;
confirmation occurs after successful reservation and issue. Neither preparation
nor adapter existence activates an origin. An accepted exact scheduler request
activates it; cancellation, terminal failure, rejected requests, poll failure
and lifetime end revoke it. This is input-origin custody, not a current
source-owned action map, revision authority, physical-device receipt or retained
Parsec execution. Those binding and gameplay requirements remain incomplete.
Plans without an installed input call retain their existing admission path;
input-origin preparation does not preempt their resource or stale-offer refusal.

The retained-DOM journey now explicitly selects a checked compact Look before
its domain snapshots. The first live attempt encountered generic browser
fallback and refused its required Look assertion before any Todo action. A
separate empty control connection exposed owner-service termination while
reading the prefix; bounded client dispatch now retires incomplete clients
without retiring the owner. Its regression uses one listener and retained
owner, followed by a valid authenticated read-only Face request.

An explicit ignored retained-DOM consumer requires the actual journey record,
exact producer source, and an immutable verified owner execution receipt. It
checks the recorded three-to-two pending transition through native rendered
pages, deterministic linear output and utterance-plan provenance. It neither
creates a fixture replacement Body nor performs an owner action. Until the
live journey and consumer execute successfully, this remains prepared software
proof, not native gesture, scanout, delivered speech or human evidence.

The installed `4cf2c44c9` retained-DOM producer completed two supplied reopen
actions and one completion on the existing three-item Body. Its stale Show
return refused without changing the immediately observed Face. The outer
capture then refused because browser cleanup changed Face identity/revision;
it did not produce the verified-execution receipt required by the consumer.
The corrected producer captures the actual post-stale Face before cleanup and
retains a separate later owner Face. Generic, compact and spacious Looks now
record source-correlated semantics, availability, disclosure, keyboard focus
and measured density. These corrected captures still require live execution.

Parsec now has checked source-owned finite map transitions and a bounded child
scan retaining their complete results. Focused software tests cover phase,
remap, stale revision, revision exhaustion, cancellation, pressure and parent
Play receipts. An actual StdHost advertisement and one whole Body Play accept
typed commands through a sealed live Fore queue. This proves the software scan
path; it does not establish a current keyboard binding, physical input, or a
Face/Show-correlated action grant. The combined keyboard/scan profile and its
private current-binding checks remain required.

The prepared input-policy evaluator checks exact authored-source reproduction
and selected Plan custody, validates generated owner contracts, and evaluates
phase/control proposals without runtime allocation. Five focused software
tests and strict presentation Clippy pass, including altered checked-source
refusal and malformed/refinement-forged frames. Proposal context and its
revision remain supplied observations; they do not mint current authority.

The separately admitted combined Parsec profile selects exactly one raw
keyboard source, one Space button source and the retained scan. A whole Body
test observes Pending followed by a software Space event, acknowledges both
source outputs without a queued command, and explicitly cancels the standing
sources. Exact current Host/Boot/offer and boundary counterexamples refuse.
This is software adapter and scheduler proof, not physical keyboard evidence.

Private completion custody is now enabled before issued Play only for that
profile. A fixed prepared ledger records exact accepted request/value identity
after successful scheduler completion. Fore delivery claims the exact source
value and retires custody only after successful kernel acknowledgement; newer
requests cannot replace older records. Cancellation and failure revoke them.
The raw keyboard Back now transfers its validated original completion value,
matching the button Back, instead of copying bytes into a different value.
Four custody tests, twenty keyboard regressions and strict Std Clippy pass.
The policy also has exact current-Host source Plan preparation with missing or
substituted offer refusal. These changes do not yet connect acknowledged scan
state and current Face/Show to an authorized Start/Shoot interaction.

The private Parsec map observer now initializes from the selected scan's exact
initial accumulator, activates only for the confirmed Body Play, and updates
only after the exact lowered states endpoint acknowledges a source result.
Wrong source node, endpoint, sequence, shape or owner law revokes availability;
successful acknowledgement preserves prepared capacities without allocation.

An explicit terminal attachment now projects the source policy's emitted action
into a semantic Face and obtains its Show through the concrete terminal Mask's
actual write/flush receipt and kernel Host Call/Show Fore completion. It retains
the real Body basis, current map and concrete execution privately. The scoped
checked Mask Plot has a separate identity from ordinary terminal execution;
bounded sequence ranges distinguish successive Body Plays and refuse exhaustion
before writing. An actual combined Body test records three source Faces/Shows
through Start, Shoot and remap while preserving the four source transitions and
input-output cancellation checks. Twenty Parsec tests and strict Std Clippy
pass; existing terminal regressions have 53 passes and one explicit ignore.
These are software terminal effects, not foreground human or physical proof.
At that checkpoint, completed input did not invoke a Face action through this
terminal route; private policy correlation and typed return/command admission
remained required. The completed-input route below supersedes that gap.

### Completed input through the retained Parsec terminal

The bounded combined Body route now consumes private completed-event custody
only after kernel Fore acknowledgement and stages the original bytes in retained
delivery history. It checks events after the entire Fore drain, including source
state acknowledgements. Both raw-key and button observations retain the private
Show epoch captured at adapter completion; that stamp grants no authority by
itself. Admission separately checks the exact retained Body Plan, issued Play,
full current Host advertisement and original input descriptor. A later accepted
request does not replace an older genuine completed event.

The concrete terminal owner evaluates the checked input policy against its
current acknowledged source map and actual Face/Show. Raw Space is not a second
Fire action; releases, missing/stale epochs and controls absent from the current
map produce no command. The source action is focused before the terminal frame
crosses write/flush and Show acknowledgement. Apply returns through the actual
checked Mask execution, whose typed correlation is checked before constructing
the native command and admitting it to the live source queue. A consumed Show
stays unavailable until the corresponding source output is acknowledged, even
when Shoot leaves the map unchanged. Full staging withdraws availability and
prepares a fresh Show after pending acknowledgements without replaying an action.
Stop guards observe cancellation without consuming its lifecycle request.

Twenty-two focused Parsec tests, twenty-five keyboard regressions and fifty-four
terminal regressions (one explicit ignore) pass. Strict all-target Std Clippy
with burn-model and warnings denied also passes. The Parsec tests include an
actual combined Body Play
with Space→Start, Space→Shoot, source remapping, refused remapped Space and
Enter→Shoot. Four actual source transitions and child Play receipts are checked;
there is no fifth command from remapped Space. Separate mechanisms cover stale
Show epochs, releases, unmapped Enter, Full recovery and inflight refusal. This
is software keyboard/terminal evidence, not physical input, a browser Parsec
journey or complete gameplay. Installed cross-Mask Todo execution, two genuine
spoken Looks, hosted acceptance and stable publication remain required for
#5355; this bounded route does not close #5369.

### Checked brief and expanded spoken wording

Two ordinary checked source expressions now receive the same finite Face-derived
wording slots. Both retain every clause from the existing aural projection;
expanded adds only source-owned connective wording. Selection preserves semantic
item order, item properties/text, unavailable action facts and unknown fallback.
Each selected clause occurs exactly once in the slot partition. Exact Face
identity/revision and aural digest bind the slots; stale context, missing status
and bounds refuse without truncating essential meaning.

Template preparation requires an independently reproduced checked Source Plan
and the exact expression offer on the supplied current Host. The receipt retains
template identity/revision/source digest, producer Plot/Plan, Face identity and
revision, selected clause/action references and wording digest. This is
computational custody, not a Source Play, spoken Show or audio receipt. The
finite text bound is 8192 bytes, with admitted canonical frames of 131072 bytes.

A fresh read-only capture from the existing retained Todo owner at source
`4cf2c44c9` supplied its actual Face to the explicit consumers. Both Looks preserve
all 76 aural clauses: brief is 4871 bytes, expanded 4918. Face/action identities,
clause references and aural digest agree. Source distinction, current exact
Host offers, altered-offer refusal, stale Face, omitted status and excess bounds
are checked. The actual owner was neither replaced nor acted upon. The template
Host is an explicitly constructed software test Host; these checks do not prove
that the installed owner selected either template. Ordinary direct spoken
opening behavior remains separate. Installed template/Mask admission, bounded
wording delivery, WAV artifacts and cross-Mask recovery still require execution;
no speaker delivery, hearing, full #5380 or #5355 acceptance is claimed.

### Template opening execution and bounded continuation preparation

An opaque current-Host template reading now retains the exact checked script,
full advertisement and Face. UTF-8/word-safe frames are at most 256 bytes and
64 frames; reconstruction must preserve every byte and its digest. Empty speech
turns and overlong words refuse before effects. Existing Tongues segmentation
and its 30-second/32-segment per-Play limits remain unchanged. The prepared
reader exposes bounded frames but grants no acknowledgement or cursor advance.

Explicit brief/expanded selections have distinct ordinary Mask namespaces and
Plans with the same typed Fore. Only internally checked source selection can
prepare their openings. Runtime entry rechecks full Host custody before Play;
ordinary public preparation cannot impersonate either reserved template Mask.
Idle speech equipment attachment selects the exact template computation offers
before publishing the advertisement and planning a Mask. The opening Show's
optional checked-template evidence preserves ordinary receipt compatibility and
records the full script plus the acknowledged prefix and finite remaining scope.
Validation replays the exact checked expression, so rehashed appended claims or
dropped clauses still refuse; documentary metadata never becomes a grant.

Five artifact-evidence tests pass, including altered source/Face/digests,
invented wording, missing coverage and false frame claims. Lossless framing and
the explicit actual-retained-Face consumer pass. Three ordinary direct-spoken
regressions pass (two environment-dependent tests remain explicitly ignored by
default). Strict all-target presentation/Std Clippy with warnings denied passes.

The explicit native-provider test selects real eSpeak and native language
coverage on one Host/Boot and consumes the freshly captured retained Todo Face
from owner source `4cf2c44c9`. Actual kernel Plays acknowledge both opening WAVs
through their exact Shows and template evidence. PCM/file bytes and hashes are
checked; opening clips have 3592144 and 3668092 WAV bytes. Missing template
offers, stale generation and ordinary preparation claiming a template namespace
refuse. The opening prefixes are 252/256 bytes; each Look still has 19 frames
remaining. This proves real software synthesis and opening artifacts, not
installed owner template selection, complete template audio, speaker delivery,
hearing or #5355 acceptance. Installed selection, ordered continuation/aggregate
receipts and fresh exact-source cross-Mask execution remain required.

### Installed checked spoken selection and ordered execution (in progress)

The owner accepts `spoken-mask admit --look brief|expanded` and
`spoken-mask select --look brief|expanded`. These finite choices select the
checked source expressions; the caller supplies no script, receipt, template
identity or Plan. Omitting `--look` preserves ordinary direct speech. `--llm`
and a checked Look are mutually exclusive.

The owner privately retains the opaque template selection alongside its route
seal. Start revalidates current Host/Face, Mask Plan, grants and exact evaluated
wording before transferring the sole Host to its worker. An actual completed
opening Play/Show mints the private execution session. `read-remaining` consumes
that session through ordered same-Host artifact Plays, advancing only after
verified completion. Cancellation or failure retires the session; supplied
receipts cannot advance it, and a missing session cannot fall back to ordinary
remaining-item reading. Opening status distinguishes its acknowledged frame
from complete-script evidence. Continuation receipts identify their checked
reading scope and do not claim speaker delivery.

Full native execution exposed synthesis overflow with both 256-byte and
192-byte frames. Provenance identifiers can take substantially longer to speak
than ordinary prose of the same byte length. The current lossless framing uses
96-byte word/UTF-8 boundaries with at most 64 frames, retaining the existing
per-Play 30-second PCM and segment limits. The actual retained Todo scripts
partition into 55/56 frames without dropping or rewriting content. The
comparison proof explicitly selects the existing 128-artifact retention pool
before Host planning so both scripts fit on one Host; ordinary owner retention
remains unchanged. This is a capacity admission, not evidence that synthesis
has completed. Revised native execution and installed-owner selection tests
remain required; no full-script or issue-acceptance claim follows from this
implementation contract.

The revised explicit native-provider proof passes with 96-byte framing. Brief
and expanded complete in 55/56 frames (4871/4918 wording bytes) on the same
Host/Boot and captured actual Todo Face. All 111 distinct Plays retain verified
WAVs totaling 177436680 bytes. The completion audit checks ordered frame text
hashes, complete wording hashes, exact Face/Show references, distinct Plays,
RIFF/WAVE extents and each artifact's declared PCM or complete-file hash scope.
The pool is explicitly admitted for 128 artifacts before planning. This proves
complete software synthesis and retained artifacts, not installed-owner Look
selection, speaker delivery, hearing or #5355 acceptance. Seven owner speech
regressions, four focused CLI/IPC tests, lossless framing and explicit
actual-Face preparation/staleness tests pass; exact committed-head repository
validation and fresh installed cross-Mask execution remain required.

Installed artifact retention can be explicitly selected with
`--speech-artifact-maximum-artifacts` and
`--speech-artifact-maximum-bytes` alongside an explicit speech equipment
selection. Both bounds are required, retained in the installation record, and
validated through the existing finite retention admission before a fresh Host
Boot attaches the pool. Absent bounds preserve the ordinary 64-artifact
default. This controls retained capacity without changing the per-Play audio
limits or silently changing a running Host. Compilation and three focused compatibility/bounds/CLI tests of this
additional installation path pass. Strict all-target product Clippy with warnings
denied also passes. Fresh exact-source release/installed execution remains
required.

### Released retained Todo comparison

Fresh Linux, browser and Handbook packages from `cd3fd71ef` passed their
supported release commands. The installed owner was upgraded while idle with
the same retained Body-state reference and explicit finite artifact limits.
Both checked spoken Looks completed through actual installed selection and
same-Host execution on one exact retained Face: brief 55 frames/4871 bytes,
expanded 56 frames/4918 bytes and 111 distinct Plays. Wording coverage and WAV
digests pass; no speaker delivery or human hearing is claimed.

The corrected retained browser producer passed generic/compact/spacious Look
and DOM mutation/stale-return checks. Its actual verified owner execution was
captured before reconstruction. The first consumer refused `InvalidIdentity`
because JavaScript rounded the numeric u64 revision; the capture also retained
the owner's exact decimal revision. The consumer now checks that decimal against
the numeric carrier and validates the original Face content identity. It does
not invent a new Face or replay owner actions. Two precision/refusal tests and
the actual retained native/linear/utterance reconstruction pass against the
unchanged producer and execution records. Product all-target Clippy with warnings
denied, formatting and patch hygiene pass. These are software browser/action
and semantic reconstruction proofs, not native scanout or audible delivery.

### Current-request Parsec terminal instructions

The private terminal prompt now evaluates the checked Parsec input policy over
its acknowledged source map and current Face/Show, then verifies the exact
accepted input descriptor against the issued Body Plan/Play and full Host
advertisement. Prepared descriptors cannot enable an instruction. The installed
Space-button realization uses the source label `Space`; the raw keyboard route
uses `Enter`. Generic Fire proposals retain their separate semantic label.

A bounded terminal write and flush precede private prompt availability and the
captured input epoch. Production input rechecks original completion custody,
the current prompt, control realization and epoch before the existing checked
Mask typed return. Consumption, remap, cancellation, stale scope and terminal
outcomes withdraw the instruction. Failed output attempts erase a possible
partial instruction and retire authority. This sidecar effect creates no new
Show and proves no physical controller activity.

The contributing `cargo +stable check -p conduit-std-host --tests` passes.
Four focused prompt tests, twelve input-custody regressions and the actual
combined Body journey pass. The journey observes Space-to-Start, Space-to-Shoot,
remap and Enter-to-Shoot instructions, current source effects and final prompt
withdrawal. Strict all-target Clippy with burn-model and warnings denied passes,
as do six checked-policy regressions, workspace formatting and patch hygiene.
This is not #5369 or #5355 acceptance. The current instruction is a terminal
sidecar; post-flush observation and cross-Mask instruction reconstruction remain
required before the Parsec specimen is accepted.

### Post-flush Parsec instruction observations

The optional Body observer now receives a Serialize-only instruction snapshot
from the private owner after current accepted scope, checked policy correlation
and successful terminal write/flush. The snapshot carries its exact Face/Show,
issued Body Play, Host/Boot/offer generation, source producer Plan/Plot, binding,
prompt-text digest and epoch. There is no deserialization or replay admission
API. Existing Show observers retain their default behavior.

Twenty-seven Parsec tests pass, including observation absence before acceptance
or flush and after cancellation. An explicit private export from the actual
combined Body journey records four post-flush instructions (Space/Start,
Space/Shoot twice and Enter/Shoot), four actual Shows, four canonical source
transition frames, four child receipt identities and final prompt withdrawal.
The export refuses to overwrite existing evidence. Std all-target Clippy with
burn-model and warnings denied passes. This is software input/terminal evidence;
it does not establish reconstruction of the instruction by other Masks.

### Parsec instruction carrier counterexample

The explicit actual-artifact diagnostic
`actual_parsec_instruction_is_lost_at_face_only_mask_boundary` passes against
the post-flush exchange above. It validates captured Face/Show/Body/Host/binding
correlations and canonical source transitions, reconstructs native scenes,
linear output and aural plans, and rejects substituted, missing and action-
rewriting binding records. This checks documentary consistency; no artifact
can grant runtime input authority.

All three reconstructions preserve the offered Start/Shoot action, but their
action-associated wording lacks the admitted Space/Enter mapping. Native
navigation still says `Enter act`; that describes generic Mask activation and
is not evidence of the source policy's current controller mapping. The same
source action survives the remap while the actual terminal instruction changes.
This is the concrete #5369 presentation/input encounter carrier gap required
by the #5355 Parsec specimen. Canonical Mask admission and the Face ontology
remain unchanged. The diagnostic proves neither a browser Show nor native
scanout, spoken delivery or completed cross-Mask instruction acceptance.
Product all-target Clippy with warnings denied and patch hygiene pass.
### Checked instruction component preparation

The authored `parsec/input-instruction` producer now emits bounded ordinary
component data outside Face. It forwards the exact input binding, checks
phase/action and mapping-revision consistency, and authors Begin/Shoot wording
for finite arcade, linear and spoken choices. Explicit withdrawal or an
unavailable proposal yields only `Input unavailable.`. The original nonempty
64-byte label contract is retained exactly; three source-emitted segments form
one fixed 256-byte wording buffer without truncation or Rust-authored phrasing.

Five checked-source tests pass: all policy phases/remaps/Looks, withdrawal,
64-byte UTF-8 custom labels, malformed/overbound/stale/cross-action refusal and
changed checked producer refusal. Evaluation/refusal retains prepared storage
without allocation. The exact-offer Std source factory test and strict
all-target presentation/Std Clippy with burn-model and warnings denied pass,
as do workspace formatting and patch hygiene.

These outputs establish computational custody and candidate consistency only.
They do not decide current availability, correlate keyboard/control policy,
grant input authority or certify a rendered instruction. Private accepted
request ownership, internal Source Fore completion and each medium's actual
supplementary effect acknowledgement remain required for the #5369 carrier.

### Actual internal Parsec instruction execution

The terminal specimen now composes an ordinary internal instruction Source and
supplementary effect sink without changing the canonical Mask Front or Face.
The private owner qualifies current accepted input, source map, Face/Show and
full Host; an opaque, non-cloneable witness is revalidated before completion.
The Source consumes the original acknowledged renderer Show value. Its actual
scheduler completion, internal Fore forwarding and supplementary effect request
retain the exact original instruction value, rather than reconstructing custody
from JSON or observation metadata.

The initial Show does not certify the later instruction. Source-authored linear
Begin/Shoot wording is written and flushed through the separate admitted effect;
the kernel consumes that exact effect completion before input becomes eligible.
Cancellation, failed output and stale scope retire custody. The effect consumes
the one-item Flow closure before retirement. The attached terminal advertises
and withdraws a separate capacity-one instruction-operation reservation, keeping
the initial terminal-output pool at one; this is a finite operation slot on the
same provider, not another terminal device. Ordinary three-gear and Parsec
five-gear scheduler profiles remain exact, sharing the production kernel.

All 38 contributing Parsec checks pass, including the actual Start/Shoot/remap/
refused-Space/Enter Body journey. A fresh retained exchange contains four actual
post-effect instructions and four source transitions. Its software fixture guard
is bounded at 30 seconds to accommodate checked preparation; the targeted journey
completed in 12.50 seconds with all original action/state assertions intact.
Native/browser/linear/spoken instruction encounters still need their own actual
transport/effect evidence. This establishes neither physical controller input,
scanout, speaker delivery nor completed cross-Mask acceptance.

The actual-artifact product diagnostic now consumes the fresh Source/effect
exchange. One explicitly prepared computational evaluator reproduces the checked
instruction wording for all observations and negative records; that fixture
supplies no runtime input authority. Invented wording refuses even after its
recorded digest is recomputed. Native, linear and aural Face-only reconstruction
still preserves the action but loses its current binding. This remains a
counterexample, not completed instruction encounters in those media.
The private post-effect observation additionally retains the original canonical
instruction frame, its checked computational producer Plan/Plot, and the exact
Source/effect node/call/request and value occurrence. It is Serialize-only data,
not a deserializable availability witness. A focused actual-owner test compares
its bytes and producer identities against the checked source; the whole-Body
journey retains four original frames (1542/1544 bytes) through the remap. Both
checks and strict all-target Std Clippy pass. These observations supply reusable
fixture evidence; their serialization cannot grant an input or another Mask.

A prepared frame inspector decodes the original ordinary instruction component
under its checked native owner and value laws, preserving canonical binding and
source segments in one fixed 256-byte buffer. All six component producer/
inspection tests and strict presentation Clippy pass; all three Looks and
withdrawal inspect without allocation. Wrong owner, malformed and overbound
frames refuse. Inspection does not reproduce producer custody, establish current
availability or acknowledge an effect; consumers must keep those checks separate.

### Captured instruction encounters across four renderers

The explicit actual-frame reconstruction check independently reproduces checked
Source output and validates original component laws before passing the retained
instruction to native local notice, a deterministic linear record and a bounded
spoken reading. Original Source/effect routes are compared with the lowered Mask;
missing, stale, malformed and rewritten instruction/binding records refuse first.
The optional browser-record export occurs only after these checks pass and retains
exact decimal revisions alongside the complete original Face and provenance.

`proof/browser/parsec-component-reconstruction.mjs` uses pinned Playwright 1.62.0
Chromium, one page and no retries to encounter the four checked records through
an independent accessible instruction component beside the existing owner Face.
Withdrawal removes the reading; the original Face action remains disabled in this
historical reconstruction and is never replaced by an instruction callback.
This is captured-occurrence reconstruction, not a current browser Show/input grant,
physical control, scanout, synthesized speech or speaker-delivery claim.

### Parsec browser Fore agency fixture

An explicit consumer of original-frame-validated Parsec records now prepares the
actual checked browser Mask for all four captured Faces. Each fixture has its own
Host/Boot, planned Mask and Play, and acknowledges its own Show before returning
the supplied Start or Shoot action through the typed interaction Fore. Exact
Body/source Play, Face identity/revision, context, target, Mask Plan/Play and Show
correlations are checked. Captured terminal metadata does not grant this browser
fixture authority, and no returned interaction is submitted to the owner.

Prepared, stale, forged, wrong-target, extra-argument and unavailable proposals
refuse before egress while retaining the pending interaction request. The
unavailable specimen derives a fresh canonical Face identity and checks the
specific availability reason. This is bounded kernel/Fore fixture evidence with
simulated browser Show acknowledgement, not an installed browser carrier,
current device binding or independently committed browser mutation.
