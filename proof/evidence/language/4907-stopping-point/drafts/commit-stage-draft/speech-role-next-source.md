# Window8 committed-role Source: minimal next schema

No existing Window8 Source produces `LanguageDependencyArc`. Existing speech consumes `SpeechTextTokenRoleRequest` and `SpeechTextTokenRoleResult`; its legacy `LanguageParserCommittedDependencyAdmission` requires a different four-token contiguous commit. Do not cast Window8 records into it.

The minimal new Language admission should retain:

- `request: LanguageParserWindow8QualifiedIndependentCommitRequest` (the exact original Source input);
- `result: LanguageParserWindow8IndependentCommitSet` (the exact admitted original Source output, correlated by retained Book execution);
- `arc: LanguageDependencyArc` (mechanically proposed representation, with full new Native correlation laws).

New correlation laws must retain result basis equal to request.previous.basis; selected result.active[request.edge.dependent] true; selected result.edge exactly equal to request.edge; arc dependent token equal to request.analysis.query.dependent.token.identity; arc dependent revision equal to request.analysis.query.context.query.snapshot.basis.analysis_revision; governor root iff original edge.head == 8; token governor revision equal to that same analysis revision and token identity equal to original qualified head.token.identity; arc relation base tag equal to original edge.relation.base; optional arc subtype `some` exactly matches original subtype material and `none` requires original empty subtype. Keep `LanguageDependencyArc`'s original root/nonroot/identity laws. Original qualified request/analysis laws retain pre-Final eligibility; add no new eligibility shortcut.

The closed Stage must bind request and result to the *same actual retained Source history*, retain the full accepted qualified dependency/lexical parent graph, and freshly admit this whole new representation. Matching independently supplied valid records alone is insufficient.

New speech projection input: that complete new Language committed admission. Its output can reuse existing `SpeechTextTokenRoleRequest`, with exact Source projections:

- analysis = original snapshot.basis.analysis_revision;
- basis = admitted correlated arc;
- choice = original request.analysis.query.dependent.choice;
- source = original snapshot.lexical.tape.source;
- token = original request.analysis.query.dependent.token.

Then execute the unchanged existing `speech/text-token-role` plot to obtain `SpeechTextTokenRoleResult`. The prepared speech owner must retain the opaque Window8 commit Book plus the new complete admission and both exact Source histories. A new Window8 committed role wrapper should expose the original PreparedTextTokenRole to coverage/prosody while retaining that owner; existing coverage currently specifically stores legacy PreparedCommittedTokenRole and therefore needs a reviewed generalization or distinct Window8 adapter with equally strict complete-material checks.

This is a proposed schema/projection requirement, not an implemented or checked Source artifact. It uses only the original committed edge and lexical membership; it does not infer syntax, IPA, timing or playback authority.
