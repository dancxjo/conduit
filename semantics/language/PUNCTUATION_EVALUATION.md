# Opt-in punctuation candidate evaluation entrances

These are unbuilt test harness preparations for the separately named v3 model.
They do not change accepted v2 defaults, model bytes, supervision or Source laws.

Run the focused entrance first, from this isolated worktree, through the owner's
coordinated Cargo queue:

```sh
CONDUIT_PARSER_PUNCTUATION_V3_REPORT=/absolute/new/greeting-report.json cargo +stable test -p conduit-language --locked --features parser-model --test parser_punctuation_selection punctuation_v3_motivating_greeting_actual_source_two_tracks -- --exact --nocapture --test-threads=1
```

One case runs in two tracks through four exact Partial revisions: `Hello `,
`Hello, `, `Hello, Travis `, then `Hello, Travis.`. The independent track parses
every prefix without commitment. The causal track rebases every active state,
retains exact committed lexical choices, resets seed scores per revision and
coalesces only exact state/choice duplicates using existing Source ranking.
It never requests early root/comma commitment: pending scope is through token3,
and an actual commitment is requested only after contiguous Source agreement
already covers that scope. The returned checked result is applied to every
current beam slot. The final period remains included to expose later refusal or
conditioning consequences. The test requires pre-final Travis-to-Hello vocative
commitment, preserves trace before asserting, and retains failure as failure.

The remaining cases have a separate named entrance:

```sh
CONDUIT_PARSER_PUNCTUATION_V3_REPORT=/absolute/new/retained-report.json cargo +stable test -p conduit-language --locked --features parser-model --test parser_punctuation_selection punctuation_v3_retained_dev_test_and_other_reserved_actual_source -- --exact --nocapture --test-threads=1
```

It runs all five DEV, five TEST and six other reserved rows in both tracks: 16
cases, 32 tracks, 86 prefix decodes. The focused case contributes two tracks and
eight prefix decodes separately, so the combined partition is the original17
cases without repeating the exact motivating-period case. The separate reserved
three-token motivating row uses space-joined typography and is explicitly a
different input. Neither entrance claims newly blind DEV/TEST accuracy. Reuse
focused proof only with unchanged relevant harness/model/Source/profile inputs;
a batch report alone does not imply that focused proof exists or passed.

Each decode is bounded to eight rounds and four active survivors. An expanding
survivor invokes ten proposal/transition operations plus legality, then at most
four lexical branches, each with branch/features and four rank/select/merge/next
steps: a conservative83 SourceGraph calls per expanding survivor, at most2656
per prefix (plus complete-survivor merges, initialization/rebase, and up to nine
agreement/frontier/commit calls). Thus focused decode orchestration is bounded
conservatively near22,000 SourceGraph calls, versus roughly232,000 for the86
remaining prefixes, not a measured runtime estimate. Numerical model calls are
separate, at most128 per prefix (1024 focused). Actual candidate/choice/legality
refusals reduce these bounds substantially. Immutable Source preparation still
runs once per named entrance, with normal prepared Source Plans reused by each operation.

Reports retain all survivor scores, heads, relation subtypes, lexical choices,
true agreement/frontiers, actual commitment applications, partial failures and
numerical receipts. This is a test-owned beam/revision evaluation profile, not
installed editor, prosody publication, audio playback, physical proof, or proof
that training gold-state metrics established real beam agreement. No reserved
output may be used to retune the candidate.
