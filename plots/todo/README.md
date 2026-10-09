# Todo Plot

## Typed application core

`live.conduit` is the typed application entry for #5264. It composes the
ordinary bounded `scan` with `todo/transition`; the selected `todo/combine`
Back implements the application meaning in `model/`. The scan owns the retained
state. No browser, speech, or storage policy participates in the transition.

The state carries a list title, revision, next item identity, and ordered items.
List identity belongs to the containing Plot/Body; the title is not an identity.
Item IDs are monotonically allocated within that list and never reused after
removal. `Add`, `SetComplete { complete: true }`,
`SetComplete { complete: false }`, and `Remove` are exact typed commands.
Repeated complete/reopen commands are idempotent; missing items refuse.

The core admits 20 items, 72 UTF-8 bytes per item text, and 64 UTF-8 bytes for
the title. Encoded state and command bounds are 1,635 and 75 bytes. The
transition Back uses fixed storage and allocates nothing during a step. Item
capacity, identity exhaustion, revision exhaustion, invalid text, and missing
items have distinct refusals; a refused transition emits no successful state.
The 64-command scan bound is separate from item capacity.

Run `cargo xtask check todo-state` for typed application, source checking,
planning, fixed-storage transition, and existing std/browser kernel conformance.
The typed sequential test adds three items, completes and reopens the first,
removes the second, and compares each Back result with the semantic transition.
It verifies the surviving IDs, order, text, completion, and revision.
Durability and Mask demonstrations have their own proof surfaces and are not
established by this core check.

The installed core replay additionally executes the unchanged `live.conduit`
on two fresh std Hosts and independent fixture Bodies. Both receive the same
seven commands: add Milk, Eggs and Bread; complete Milk twice; reopen Milk;
remove Eggs. Each emitted state is checked against an explicit expected state,
decoded and re-encoded, and compared byte-for-byte across the replays. The final
state has revision 6, next ID 4, and open items `task-1` Milk and `task-3` Bread.
Repeated completion has identical state bytes and revision. Missing-item remove
and completion produce terminal failure after the three valid adds, with no
state output for the refused command; empty text refuses before encoding.

`cargo xtask check todo-state` prints `TODO_CORE_REPLAY_RECEIPT` and
`TODO_CORE_REFUSAL_RECEIPT`, retaining Source/checked Plot, exact commands and
state digests, selected Plan/Play, child invocation correlation and terminal
Sign. These are independent replay fixtures, not a Host handoff or persistence
demonstration. The [core replay packet](../../proof/todo/core-replay/README.md)
records the expected ordered states and exact digest trace.

The serialization boundary for #5265 is the bounded binary Form under exact
Info Kinds `conduit.todo/state@1` and `conduit.todo/command@1`, decoded only by
their owning checked codec. Encoding, decoding and semantic admission refuse
malformed, oversized or invalid state; rendered JSON is not this wire contract.
The resource envelope must separately bind Body/list, selected generation,
provider, authority and acknowledged read/write evidence. These pure bytes and
digests alone do not establish durable publication.

## Earlier generic JSON composition


`live.conduit` authors a bounded `scan` over the exact Todo state and command
Info Kinds. Its `todo/transition` Plot wires the combine Kind's exact ports;
the Host selects the Back for that leaf Kind. During source preparation, the
Todo semantic owner admits its
validated empty-list Form with `admit_empty_todo_initial`; expansion rejects a
missing or wrong-Kind literal. This proves an authored initial accumulator,
separate from installing the combine Back or connecting a live Body/Mask route.
The scan admits at most 64 commands in one Play; this lifetime action bound is
distinct from the state's 20-item capacity.

`main.conduit` composes `todo/state-step` and `todo/snapshot` through their
fronts. Task records are application data with `complete` and `text` members.
The reusable `json/collection-step` operation knows only JSON collection edits;
it contains no task, browser, persistence, or renderer logic.

For stable item identities, use the generic keyed commands through
`todo/state-step` with records shaped like
`{"complete":false,"id":"task-7","text":"Buy milk"}`. `append-unique`
accepts `{"key":"id","op":"append-unique","value":<record>}`;
`set-field-by-key` accepts
`{"field":"complete","key":"id","match":"task-7","op":"set-field-by-key","value":true}`
for complete or `value:false` for reopen; `remove-by-key` accepts
`{"key":"id","match":"task-7","op":"remove-by-key"}`. The caller must
allocate a stable, unique ID. Keyed commands refuse missing or duplicate keys;
setting an existing value twice is idempotent. They preserve array order and
the generic JSON bounds. Todo record shape, text constraints, and command
authority still require application-level validation before a live Face offers
these actions.

Run the current deterministic proof with:

```sh
cargo xtask check todo-state
```

This checks and expands the authored plots, plans their ordinary std and
browser host offers, and runs add → toggle → remove through the production
kernel in Rust tests. Each next request
uses the preceding kernel-produced snapshot. Source and stdout sink are explicit
test fixtures; they do not perform the state transition. An unknown index refuses
with detail `105` and produces no success snapshot.

The inherited JSON profile admits at most 32 array items, 128 total nodes,
8 levels of nesting, 1,024 bytes in one string, 2,048 total string bytes, and
4,096 encoded bytes. These limits apply together to the complete request and
result. The proof explicitly plans 4,096-byte, capacity-one cords. Actual task
capacity can be lower than 32 because records consume multiple JSON nodes and
the command contributes to the request bounds.

This earlier JSON composition is an executable state-transition slice.
The browser runtime also tests admitted resource publish/read requests and
restores only after a matching storage completion. Those Rust tests supply
storage completions; they do not establish durable browser storage across
reloads. A complete Todo interface, application-specific validation, causal
user interaction, and a manual application entrance remain separate work.

`todo/summary` configures the reusable `json/boolean-summary` operation with
field `complete`. `todo/command-summary` composes an edit, summary, and encoding
through their fronts. Its snapshot reports `false` (remaining), `true` (completed),
and `total` counts. Missing or non-Boolean completion fields refuse with distinct
details `123` and `124`; an empty collection reports three zero counts. The same
operation counts an arbitrary configured Boolean field outside Todo.

`todo/restore` decodes stored snapshot bytes through the ordinary JSON operation.
`todo/restore-summary` consumes it as a gear and derives counts through the same
summary plot. The deterministic proof supplies the actual preceding edit output
to this restore front and refuses corrupt JSON or invalid completion fields.
This proves semantic restore and the runtime resource-operation boundary;
durable storage across a real restart remains a separate acceptance claim.


### Selected checkpoint continuity

Run `cargo xtask check todo-durability` for the bounded typed checkpoint and
installed-owner proofs. The continuity test adds three items and completes one through ordinary
Todo checkpoint Plays, admits a second Host with an authenticated invitation,
and hands off retained biography, source and lineage receipts to its fixture
installation. Both installations explicitly select the same local provider.
After discarding the first owner's display cache and retained owner directory,
the second Host Boot reads the exact generation through a fresh Plan, Play,
Host Call and terminal Sign. Body identity, one Birth, list state and item IDs
survive. Removing the checkpoint produces a signed failed read with no verified
state; restoring it permits a verified retry of the selected generation.

The test emits `TODO_CONTINUITY_RECEIPT` with before/after lineage and the failed
read. Companion tests cover corrupt, stale, wrong-namespace and unauthorized
reads, cancellation, resource exhaustion and failed publication. An acknowledged
filesystem sync is the provider's durability boundary; this suite makes no
claim about a physical device surviving power loss. The handoff is an explicit
local fixture, not remote transport or a rendered Mask demonstration.

After an interrupted next action, explicitly reselecting the preceding published
version with `conduit host service install --selected-todo-checkpoint-root …
--selected-todo-checkpoint-version …` permits a fresh verified read on restart.
Use the exact version from the retained successful write/read receipt. Selecting
the unfinished candidate does not fall back to an older list, and a retained
receipt never substitutes for reading the selected checkpoint bytes.
