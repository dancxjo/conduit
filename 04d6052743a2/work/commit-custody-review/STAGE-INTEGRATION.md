# Minimal frozen-file follow-up (not applied)

1. Add a private module declaration for `parser_session_window8_commit_candidate` in the same closed harness that currently includes the other private modules. Public lib wiring remains deferred.
2. Factory preparation must reserve one complete Native family with roots `LanguageParserWindow8IndependentCommitSetProposal`, `LanguageParserWindow8IndependentCommitSet`, and `LanguageParserWindow8QualifiedDependencyAnalysis`. Existing four-family partition does not promise all three roots in one owner; do not assume its observation family suffices. The extra family requires its complete retained/preparation/conversion/static receipt in the aggregate before construction. Alternatively introduce cross-family refinement readiness after a separate audited change; current refinement expects both roots in one family.
3. Prepare all `PreparedWindow8CommitSlot` instances before revision ingress. Sum slot declared preparation/retained ceilings, Vec slot storage and family incoming-owner ceiling before the first preparation; the slot itself checks its complete refinement+buffer+inline ceiling before allocating. Each slot is single-use.
4. Add `Window8StageRefusal::Commit(Window8CommitRefusal)` and `Window8RevisionStage::admit_independent_commit(source_execution, slot, familyRc, maximum_source_replays, maximum_native_conversion_requested_bytes, epoch, model_calls)` patterned exactly on `admit_qualified_dependency`:
   - reject poisoned stage;
   - `book.can_admit(slot.candidate_capacity())`;
   - `CommitParents::resolve(&book, source_execution)`;
   - check parents source count and candidate bytes against slot capacity;
   - obtain conversion bound through a short immutable family borrow, then release it;
   - reject the complete conversion ceiling before any Source replay;
   - for every parent, resolve its fixed PORTS metadata and invoke `guard.targets().source(port)?.replay(history)`; do not retain any family borrow across replay;
   - consume `slot.admit(&book, &parents, &mut *family.try_borrow_mut()?, maximum_native_conversion_requested_bytes)`;
   - `book.push_admission(admission.into_book(), epoch, model_calls)`;
   - every error poisons stage and cancels all targets.
5. Closed Session must run original initializer Source query from the actual initializer basis, then call this admission path. A qualified commit requires an accepted qualified dependency Analysis, its actual original anchor Source, and the exact latest privately retained commit set. Compose the full original request with existing prepared query bank, execute original fixed commit Source, then call this admission path. No string/ordinal reconstruction and no contiguous frontier increment.
6. After single Book publication, `OwnedWindow8IndependentCommit::from_published` retains the original complete book and the exact accepted commit admission with explicit inline-owner budget. It is not a legacy `LanguageParserCommittedDependencyAdmission`. Speech needs a new exact Window8 committed role Source/admission with original lexical/analysis/edge custody rather than casting into its old four-token contiguous contract.

Current candidate module typechecks against the coherent closed SDK (metadata only). No positive commitment runtime/Source replay test has run. Initial-only admission deliberately refuses rebasing or initializing over predecessor ownership; successor preserving/rebasing exact independent facts is separate required code.
