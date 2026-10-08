# Todo state slice

`live.conduit` authors a bounded `scan` over the exact Todo state and command
Info Kinds. During source preparation, the Todo semantic owner admits its
validated empty-list Form with `admit_empty_todo_initial`; expansion rejects a
missing or wrong-Kind literal. This proves an authored initial accumulator,
separate from installing the combine Back or connecting a live Body/Mask route.

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

This is an executable state-transition slice, not the finished application.
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
