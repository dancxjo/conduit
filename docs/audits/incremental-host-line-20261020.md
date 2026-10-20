# Host, Boot and Line incremental/cold Plan conformance

For #5571 under #6078, source `3fc4f7ce7e273a01301d7d4682b4621acd6d778a`
extends the existing ordinary-Plan oracle. No production cache or Planner code
changes. The earlier cost/resource/authority cases remain credited in
[incremental-real-plan.md](incremental-real-plan.md).

The pulse fixture now declares exact Host and Boot content dependencies alongside
its existing advertisement, capacity, authority and forecast facts. Replacing a
Host or Boot at the same fact generation invalidates only its candidate, reseals
Plan and Fragment identities, and preserves the same checked/expanded Source.
A removed provider's candidate and facts are omitted from the next input set;
the surviving candidate reuses its unchanged real Plan without evaluation.
Returning the removed provider with a new Boot invalidates its retained entry.
This explicitly exercises the caller's current candidate set, not automatic
provider discovery or mutation of an installed Plan.

The separate Line oracle reuses the existing checked three-Gear math fixture,
ordinary `plan_with_line_offers`, and finite two-entry incremental cache.
Local-only work is evaluated against its one Host and no Line facts. Cross-Host
work is evaluated against both Hosts and the exact offered Line. Candidate
semantic identity is the actual checked plot identity. Missing provider/Line
availability is explicit content, including an absent optional offer; generation
stays one while SHA-256 content changes. The cache only sees these exact declared
dependencies; the ordinary planner receives the corresponding actual inputs.

Changing Line/binding identity reseals both fragments without changing any
placement or its resource contract. Missing/unavailable Line, stale peer Boot,
zero payload admission, and remote-provider loss reject cross-Host work while
unchanged local work remains admitted and reused. Repeated refusal is reusable;
restoring the exact initial facts recovers the initial content-derived Plan ID.
At every step the cold and incremental disposition, cost and selected identity
agree. Every admitted Plan passes ordinary verification, including the old
retained Plans after replacement/loss. The archive includes nine JSON records
with complete old/new/surviving Plans, expected Sign contracts and exact refusal
reasons. Expected Sign contracts are not emitted execution Signs.

Validation on the source above: 18 tests pass across `incremental_real_plan`,
`incremental_line_plan`, `incremental_planning` and `external_fore_lines`.
Focused rustfmt and `git diff --check` pass. Internal reproduction:

```text
cargo test -p conduit-planner --test incremental_real_plan --test incremental_line_plan --test incremental_planning --test external_fore_lines -- --nocapture
```

The [summary](evidence/incremental-host-line-20261020/summary.json) hashes the
[full test log](evidence/incremental-host-line-20261020/tests.log.gz).
This is modeled admission conformance, not physical provider execution or a
performance benefit. The test costs select deterministic winners; they are not
fresh measured replacement savings or a hysteresis policy.

#5571 remains open: its original old-outcome custody criterion needs the real
kernel/Host lifecycle proof owned by #6102. That issue must bind these distinct
replacement Plan IDs to new Play IDs while retaining old output/Sign identities,
terminal custody, resident resources, cancellation/drain and uncertain external
delivery without replay. A retained immutable Plan alone does not prove that.
#6078 retains publication follow-through; no Candidate, Integration or accepted
main artifact is claimed here. Direct dev publication follows the task's explicit
workflow override.
