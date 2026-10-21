# Resident Patchbay typed-boundary inventory

Implementation prerequisite for #6201 under #5358/#5356. Observed source on
`codex/native-local-progress`; this is an inventory and migration constraint,
not a new accepted Fore or an implementation/acceptance claim.

## Current production route

`plots/patchbay/main.conduit` wires `input/application-event` →
`application/retained(application = application)` → `face/application-view`.
`semantics/catalog/src/application.rs` owns these exact contracts.

| Boundary | Input | Output | Lifetime |
| --- | --- | --- | --- |
| Application event source | none | `event`, application-event value | open Flow |
| Retained application | `event`, application-event value | `view`, application-view value | open Flow |
| Application view presentation | application-view value | presentation effect | existing admitted Host operation |
| Canonical `face/interaction` | `face: face/face@1`, `show: face/show@1` | `interaction: face/interaction@2` | closing Flow |

The last row is the existing canonical contract from
`semantics/face/src/contract.rs`; it is not interchangeable with the first row.

Browser installation (`targets/browser/runtime/src/installed_browser/application.rs`)
admits 4,096-byte events and the existing maximum application view. Its operations
are `browser-application-next-event@1`, `browser-application-apply-event@1` and
`browser-present-application-view@1`, under the `conduit.host/` namespace.
The retained executor (`plot_runner/application_state.rs`) treats `[0]` as an
empty initial observation before applying events and returns `output.view`.

Native (`targets/conduitos/src/native_workset/application_delivery.rs`) admits
128-byte events and 3 KiB views, selects TextInspection, and uses the separately
named application next-event/apply-event/present-view Host operations. Native
FrontDoor projects the selected exact graph into canonical Face in addition to
that application route. The browser resident port defaults to InlineGraph.

## Required migration decisions

1. Produce the initial authoritative Face before requiring a user interaction.
   Do not cast the existing `[0]` sentinel into an empty FaceInteraction, or
   create a Face/Show feedback cycle that cannot produce its first Face.
2. Feed a genuine acknowledged current Show into interaction admission. An
   available planning record or a legacy event revision is insufficient.
3. Account for the canonical interaction Flow's closing lifetime. Retiring or
   replacing a Show must not leave a perpetually open legacy event stream able
   to dispatch against its successor. Pressure, cancellation and terminal
   outcomes remain distinct; do not add implicit retries.
4. Admit complete typed input and Face storage/Host-call bounds explicitly.
   Preserve the native 128-byte/3 KiB legacy admissions until removed; do not
   silently widen them or reinterpret them as canonical Face capacity.
5. Preserve actual resident operations and source identity. Migrate production
   consumers before removing InlineGraph; unrelated Tour/Tutorial routes remain
   independently supported. Shared graph projection is reusable meaning, not
   completion of this Fore migration.

No CI, platform journey, image rebuild or proof campaign was run for this
inventory. Existing #6201 and parent acceptance remain open.

## Exact resident owner binding

Canonical browser observation now resolves the foreground resident's retained
placement through its exact Plot Plan and fragment into the composed local
kernel NodeId during preparation. Runtime reads address that bound node only;
they never substitute another application by matching its type. Ambiguous
placements and externally managed fragments have no local observation context.
The same owner identity must anchor the eventual typed ingress. A NodeId is
local to this admitted scheduler/Play, not portable identity or permission to
submit a command. This binding does not itself admit interaction transport.

## Canonical owner callback

`PatchbayApplicationPort::apply_face_interaction` accepts only the canonical
`FaceInteraction` byte encoding and its existing 8 KiB ceiling. It decodes,
admits against independently supplied current owner Body/basis/Face/Show, then
executes through the shared retained-state path. Empty input, the old `[0]`
observation sentinel, JSON application events and trailing/oversized bytes are
not alternative encodings. The callback returns typed owner requests and does
not dispatch them. It is deliberately separate from `apply`/`apply_event`.

This completes the reusable owner callback, not its transport admission. Browser
and native must still select an exact typed input port, reserve canonical input
and output storage before Play, retain acknowledged Show evidence, and invoke
this callback only at that admitted boundary. No route may be inferred from the
presence of this Rust method or from the old event Host Call.

## Selected canonical input Fore

The portable domain now owns `patchbay/accept-interaction` with one input,
`interaction: face/interaction@2...|`, no outputs or startup/configuration
parameters, one queue item and the existing 8 KiB canonical interaction bound.
The separate `plots/patchbay/interaction.conduit` forwards that closing Fore;
it contains no Host, owner address or runtime resource facts. It is a migration
Source, not the resident entry point selected by the current product.

The browser `patchbay_ingress` preparation primitive offers that exact semantic
Kind and a distinct bounded Host Call, with one in flight and zero output bytes.
It reuses the existing finite Host Call sink verb: no initial sentinel, one
pending call, input closure completes, failed/denied outcomes propagate. Exact
preparation rejects legacy event typing, open flows, changed implementation and
Host Call facts, configuration, capability identity and widened queue limits.

The primitive is intentionally absent from ordinary profile advertisement and
factory registration until an installing owner binds the exact resident actor,
reserves its typed cord/Host Call storage, and retains its genuine current
Face/Show. A Host Call handler must invoke `apply_face_interaction` and retain or
dispatch its typed owner request with an admitted outcome; it must not discard
an Edit/Mask request after changing application revision. No initial Face is
required on this command stream: the existing read-only observation produces
that first meaning. Owner-controlled publication and explicit replacement or
retirement delimit each Show's interaction stream; no implicit retry crosses a
replacement. Wiring, publication and consumer retirement remain unfinished.

## Sealed ingress Plan and local lowering

`PlannedPatchbayIngress::prepare` checks and expands the migration Source,
selects from the supplied Host advertisement through the ordinary planner,
validates the exact selected Back and lowers the sealed local fragment. It
requires an already offered Back; it never inserts one into the advertisement.
Its input Fore explicitly selects one ordered queue item and 8,192 bytes.
Preparation checks the closing canonical type, exact local shape and absence of
an external Line. The returned Plan/lowering retain identities and finite
storage requirements; they do not allocate a running scheduler or reserve
resources on behalf of the parent Body Play. Subsequent installation must use
these exact tables and storage budgets, bind the current owner and acknowledge
its real Show before enabling interaction.

## Prepared kernel transport

`PreparedPatchbayIngress` now installs the sealed Plan's one node, cord, routes
and Host Call bindings in the existing fixed kernel. Preparation allocates one
8 KiB value slot, fixed event/remote Sign storage and fixed pending-call tables.
One submitted frame closes this transport's input. Exact bytes become readable
only after the planned kernel Host Call is emitted. A consumed transport cannot
resubmit, including after failure; a successor requires explicit preparation.
The caller supplies the actual Host Call outcome, and the sink admits no output.

This is a transport preparation primitive, not a second retained application or
an automatically installed Body child Play. The parent must still charge its
storage, bind owner/Play identity and current Face/acknowledged Show, invoke the
owner callback at the emitted call, and preserve resulting owner requests.
Semantic decoding/admission belongs to that callback; transport delivery alone
does not authorize the bytes. A failed/denied/cancelled call remains terminal
failure. The common sink verb was extracted from `back.rs` to its own module;
its cancelled completion now propagates `FailureCode::Cancelled` explicitly.

## Kernel-to-owner dispatch

`dispatch_to_owner` borrows bytes only from the prepared kernel's actual pending
Host Call, invokes the existing canonical callback against independently
supplied retained owner/Body/basis/Face/Show, then completes that call. Owner
refusals remain typed in the returned receipt; owner-scope refusal maps to
Denied, exhausted revision to identity capacity failure, and other refusals to
failed input. The receipt is marked must-use and retains the typed Edit/Mask
request even when transport completion reports uncertainty. It never dispatches
that effect, creates another application instance or converts a legacy event.

This connects the reusable transport and owner callback, not the current Body's
production installation. Its caller must bind the actual resident port, charge
parent storage, retain the genuine acknowledged Show and handle the returned
request/outcome. Local CPU fixtures exercise the complete kernel/model path;
fixture Shows do not establish page Host acknowledgement or installed product
behavior.

## Revocation for Show/Play lifetime changes

`PreparedPatchbayIngress::retire` revokes access before cancelling the kernel.
After revocation, admission, pending-input access and late completion refuse;
`dispatch_to_owner` therefore stops before invoking the model. Ready and pending
transports can retire, and successful retirement is idempotent. If cancellation
fails, revocation remains set, so the old stream cannot reopen. This revokes the
transport only; a domain request already returned in a dispatch receipt remains
the owner's responsibility. The eventual Show replacement/Body retirement
caller must invoke this operation; current product wiring remains pending.

## Retained acknowledged owner turn

`PatchbayOwnerTurn` moves an owner's current Face/available Show and unused
prepared ingress into one finite window. Binding checks Face/Show validity,
Body and contribution correspondence and any required interaction context. The
domain's existing contribution predicate is reused through a narrow public
re-export; it checks scope correspondence and grants no authority. Submission
still invokes current retained model validation at the actual kernel call.

The turn stores its complete typed dispatch receipt before returning, so ignoring
that return cannot discard an Edit/Mask request or overwrite it with another
action. Retirement revokes ingress while preserving any prior receipt. Receipt
ownership transfers once; transfer does not reopen the consumed turn. An owner
must retire its old turn before installing a successor. These primitives do not
create a genuine Host acknowledgement or install/charge a current Body route.

## Compose the ordinary Mask and owner turn

`PatchbayPublication` owns a prepared ordinary browser Mask and prepared ingress.
It binds an owner turn only after the Mask accepts a complete exact Host
acknowledgement. Interaction preflights the current retained owner contract,
then uses the Mask's actual emitted typed FaceInteraction bytes through the
planned ingress kernel and stores the owner receipt. Retirement revokes the
return route and preserves its result; it does not claim pixels were erased or
an already returned domain request was cancelled.

The local composition check now traverses two source-owned turns: select a Plot,
then project a fresh Face/Show and request Edit of an exact subject. It uses the
actual checked resident Source/prepared application, ordinary Body lifecycle,
Mask kernel and ingress kernel. Host acknowledgements remain CPU fixture input;
there is no browser painting or normal Body route installation claim. The
installing product still must charge/select both Plays, bind the actual current
resident, publish real effects and handle owner requests before legacy retirement.
