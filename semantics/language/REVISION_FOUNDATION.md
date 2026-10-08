# Exact revision lineage and lexical availability

This extracts a small foundation from PR #5196 for #4907. It adds no learned
model, training artifact, speech renderer, protected-set owner or public Session.

`LanguageTextRevisionLineage` retains both complete revisions. Its Source laws
admit unchanged material, or an exact same-text/same-language successor with a
new revision identity, matching prior revision and sequence, and a checked
sequence increment. `prepare_text_revision_lineage` admits these metadata laws.
Before publishing changed text, consumers still use `validate_text_revision`
for Unicode scalar prefix preservation and their finite revisable frontier.
Lineage metadata alone does not authorize stability, commitment or playback.

`LanguageParserAvailableLexical` admits a prefix from a tape containing at most
four token occurrences. Selected occurrences must be complete, have lexical
candidates, and carry the tape's exact text/revision and ordinal identity with
Unicode scalar spans. This is the original four-token profile, not an assertion
that four tokens suffice for general English or the later Window8 parser.
`language-parser-availability` derives waiting and final-input flags. An empty
prefix waits; final input requires all tape tokens and explicit source finality.
Availability does not imply stable dependencies or parser commitment.

The original #5196 availability-state, action-mask, growth and scorer operations
are deliberately separate follow-on slices. The public schemas are installed
in the Language catalog; the derivation's exact checked program is generated
alongside the existing Language programs.

Internal validation commands, once the integration owner releases the shared
Cargo target:

```sh
cargo +stable test --locked -p conduit-language --test revision_lineage \
  --test parser_availability_foundation --test text_revision --test lexical
cargo +stable check --locked -p conduit-language --lib
cargo +stable clippy --locked -p conduit-language --lib \
  --test revision_lineage --test parser_availability_foundation -- -D warnings
```

The Source checker and generated Native bindings run as part of Cargo's Language
build. No extra model files, F32/affine owners, Burn runtime, optional parser
features, or #5196 shared-infrastructure changes are included. These commands
have not yet been run on this extracted tree; source review and diff checks are
not runtime evidence. Keep #4907 open.
