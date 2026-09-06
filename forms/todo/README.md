# Todo state slice

`main.conduit` composes `todo/state-step` and `todo/snapshot` through their
Faces. Task records are application data with `complete` and `text` members.
The reusable `json/collection-step` operation knows only JSON collection edits;
it contains no task, browser, persistence, or renderer logic.

Run the current deterministic proof with:

```sh
cargo xtask check todo-state
```

This checks and expands the authored Forms, plans their ordinary std offers,
and runs add → toggle → remove through the production kernel. Each next request
uses the preceding kernel-produced snapshot. Source and stdout sink are explicit
test fixtures; they do not perform the state transition. An unknown index refuses
with detail `105` and produces no success snapshot.

The inherited JSON profile admits at most 32 array items, 128 total nodes,
8 levels of nesting, 1,024 bytes in one string, 2,048 total string bytes, and
4,096 encoded bytes. These limits apply together to the complete request and
result. The proof explicitly plans 4,096-byte, capacity-one Cords. Actual task
capacity can be lower than 32 because records consume multiple JSON nodes and
the command contributes to the request bounds.

This is an executable state-transition slice, not the finished application.
Snapshot encoding is not persistence. Admitted Resource write/restore,
application-specific task validation and limits, summary, semantic browser
presentation, causal browser interaction, Patchbay inspection, and a manual
application entrance remain open.
