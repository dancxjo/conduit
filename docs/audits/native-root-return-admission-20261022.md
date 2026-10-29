# Canonical Patchbay Root return admission

Refs #6201 and #5994; delivery #6255. This completes the previously missing
CPU integration of the production Root broker with substituted service returns.
It does not execute a protected guest or prove hostile-code confinement.

`canonical_root_returns` compiles the actual `protected_application::Owner`
and its publication/consume/admission/quarantine implementations. A test-only
transport runs the real Application State, staged preparation, command codec,
Graph reply phases and typed reply decode. Only after that complete exchange
can the harness substitute one decoded Consume reply. Test publications use
checked real native Keyboard canvas and Memory Lantern Source graphs, sparse
native target slots 1/3, and a valid fixture MaskShow. They are not fake runtime
Plan/Play or scanout witnesses. The fixture is excluded from product builds.

Six tests pass, including two genuine paths and 19 substituted cases:

- Genuine selection commits target 3 once, changes the exact graph, consumes
  its publication and refuses replay without another service call.
- Genuine Edit admits the exact predecessor graph/subject proposal without
  changing Root selection.
- Eleven selection-return alterations cover old/absent/out-of-range targets,
  wrong slot, old/skipped/zero/max revisions, invented proposals, inconsistent
  refusal and the wrong reply variant.
- Four forged successes cannot override stale Show, unknown action, wrong
  target or stale Face revision in the original input.
- Three altered Edit returns cannot change expanded Plot, subject or target.
- A coherent dishonest refusal cannot move Root selection. When the actual
  fixture actor has privately changed target, its next projection fails the
  Root identity check and quarantines the service.

Every invalid returned success checks unchanged Root selection, inaccessible/
retired graphs and publications, service quarantine, and refusal of replay
without another entry. The coherent refusal case checks unchanged accessible
Root graph until the subsequent projection detects drift. This is effect
admission at the existing boundary, not a second application reducer.

The positive control initially failed because the harness redundantly used
the generic data serializer after decoding the real transport reply. Removing
that extra, incorrect conversion restores the intended decoded-return seam.
The initial failure remains in the local diagnostic log; its negative cases
were not counted as successful admission proof. Initial standalone module
lookup failures were fixed with explicit module paths. No production guard,
authority check, runtime limit or transition was relaxed.

Supported native unit profile invocation:

```sh
cargo +stable test --locked -p conduitos --features native-compositor --test canonical_root_returns
```

The native-compositor feature requirement matches this proof surface.
Freestanding `cargo +stable check --locked -p conduitos --lib --target
x86_64-unknown-none` passes after the explicit module-path change, with 46
existing warnings. The hosted test build retains the existing unused ingress
`plan_id` warning; strict combined-head lint acceptance is not claimed.
Workspace formatting and whitespace checks pass. Unrelated broker entry
points remain compiled through function references rather than new lint
suppressions or fabricated enactment.

The checkpoint describes logs retained under
`evidence/native-root-return-admission-20261022/` for the final six-test
result, freestanding result and earlier harness failure. That directory is not
present in this Git checkout. The
[real product packets](native-controls-final-proof-20261022.md) retain their
original clean Source `3217e783b`. These later changes affect only test selection,
explicit equivalent module paths and this proof; they do not relabel captured
products. Actual protected malicious guest/scheduler/heap proof, the unresolved
earlier Crèche refusal, >12-Cord product workload and combined-head acceptance
remain separate requirements. No parent issue is closed here.
