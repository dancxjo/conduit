# Single-survivor rebase replay preparation

`parser_rebase_replay::PreparedParserRebaseProgram::prepare` parses the immutable
existing Source program once. Its shared `replay(&self, preparation, observed)`
method can serve all survivors and later epochs without repeating that parse.
Every call still performs full Native input admission, bounded evaluation and
exact observation comparison; it shares no mutable runtime. The returned
`PreparedParserRebaseReplay` owns an existing
`PreparedParserRevisionRebase`, evaluates the one existing checked
`language-parser-revision-rebase` expression, compares the entire raw numerical
state against an observation, then admits the exact Source-selected fields as
`LanguageParserState`. It retains canonical input/output, exact program hex and
the same generated owner Source identity as commitment preparation.

This adds no authored Source, Type or law and changes no SourceID. Build extraction
adds only `parser_rebase_program.hex`; it neither duplicates nor reruns the eight
commitment programs. Input is bounded65536B, generated Native request laws run
before evaluation, output is bounded65536B, and all admitted state constraints
run after exact observation comparison. The Native field adapter selects no
linguistic result; every field comes directly from the Source result.

The receipt covers ONE survivor, not a complete beam, live currentness, lexical
choice custody, epoch handoff or installed execution. A runtime observer must
bind actual Plan/Play sequence/input/program identity independently. A future
owner must correlate every active survivor and committed lexical choice against
its private prior state before an epoch transition. This module cannot mint
CurrentCommittedDependency and changes no AnalysisSession/Core revision law.

Prepared tests (unbuilt) are Native-only fixture expectation/replay checks in
`tests/parser_rebase_replay.rs` and a real existing Source Plan/Play observation
comparison in `tests/parser_revision_protection.rs`. The former explicitly makes
no installed execution claim and checks stale basis/wrong output refusals.

Integration from the isolated branch requires the new module/test files plus
two small root-reserved wiring changes: append `pub mod parser_rebase_replay` to
lib.rs, and add `(language-parser-revision-rebase, parser_rebase_program.hex)` to
the existing build.rs pure-expression extraction list. Those wiring edits exist
only in this isolated worktree for review; do not replace root's complete files.
Then owner-coordinated validation can run the new Native test first, followed by
the affected existing actual rebase Plan/Play test. No builds were performed
while preparing this slice.

Focused repository entrance: `cargo +stable xtask check parser-rebase-replay`.
This runs the reusable receipt/refusal tests and actual public Source correlation.
