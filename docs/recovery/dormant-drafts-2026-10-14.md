# Current-source review of five dormant drafts

This review addresses [R1 #5662](https://github.com/dancxjo/conduit/issues/5662)
under [#5388](https://github.com/dancxjo/conduit/issues/5388). It compares the
preserved original changes with exact development source
`f9419414896bbf1fba35a5b2ec02f850e6ebbd9a`. It does not revive the omnibus PRs,
change their heads, or establish acceptance of their historical execution.

The [machine-readable inventory](../../proof/recovery/dormant-drafts/2026-10-14.json)
retains all five heads, original PR bases, SHA-256 identities of full binary
diffs, every changed path, byte-identical/different/absent current paths and
synthetic three-way conflict paths. The five remote recovery heads were checked
against the original PR heads. They remain under
`archive/dormant-drafts-20261010/pr-NUMBER`; no recovery branch was deleted.

| Original PR | Preserved head | Changed paths | Identical to current dev | Three-way conflicts |
|---|---|---:|---:|---:|
| [5196](https://github.com/dancxjo/conduit/pull/5196) | `c02b31605203300773bc40674be186b7e8655453` | 370 | 8 | 23 |
| [5273](https://github.com/dancxjo/conduit/pull/5273) | `6806f08fbb07723237441b97c93cebf5d1e72138` | 150 | 28 | 23 |
| [5335](https://github.com/dancxjo/conduit/pull/5335) | `ebccf03a22479fbe1deb31c1140fb6259ee6cb7a` | 7 | 0 | 5 |
| [5337](https://github.com/dancxjo/conduit/pull/5337) | `a803306e6abad04faba7da2861a6aaf0980fef47` | 10 | 0 | 8 |
| [5336](https://github.com/dancxjo/conduit/pull/5336) | `407e746551929eb0081cb4c52d7353ef929a91e6` | 13 | 0 | 1 |

The counts describe the original PR deltas, not only their final cleanup
commits. An absent old path is not automatically absent behavior: current
contracts, renames and independent implementations must be reviewed before
porting. A clean textual merge is also not compilation or acceptance proof.

## Integrated pieces and remaining candidates

In 5196, integer categorical execution, model-resource admission and revision
lineage are among the eight exact current matches. Those generic foundations
must be reused. The original learned v2 model compatibility admission remains
a recovery candidate: `parser_model_selection.rs` retains exact lexical,
feature, availability, legal-action, content and signature identities. It owns
preparation rather than parser policy. Current development has its own checked
parser, finite beam and availability foundation; none should be replaced by the
old Session/Window8/program-bank substrate.

In 5273, the 28 exact matches include generic acoustic quantities, rate
projection, trajectory components and selected IPA admission/tests. Decibel
code has advanced separately. The old common syllable component still offers a
useful finite membership/nucleus/stress and source-reference slice in
`syllables.conduit`, `src/syllable_intent.rs` and their tests. It is not a license
to pull gesture rendering, correspondence or acoustic timing into that slice.

In 5335, the exact-Type immutable record-access recipe and its allocator/foreign
Type tests remain distinct from the current per-field borrowed record reader.
Its old validated-extent helper and prepared-family binding files are absent.
A port must use the current canonical encoding and preserve allocation refusal
before preparation; importing the old whole binding generator is premature.

In 5337, the imported-owner descriptor bridge retains complete Type, law,
contract and child-edge checks. Its prepared-family metadata prerequisite is
absent from current development. Its final readiness/ceiling fix belongs to the
same full ten-file delta, not a separate replacement for that delta.

In 5336, model-resource preparation adds requested/retained payload storage
reservations and one-under refusal before digest/adoption. Generic resource
admission is already integrated, but this storage receipt is not present under
its original API. The draft expressly excludes root/Arc bookkeeping and stack
from its payload counters; it cannot certify complete inference memory.
Three original adoption/refusal tests are ignored and require the original
`CONDUIT_FARGAN_MODEL_FIXTURE/resources/f32.bin`. Neither a synthetic replacement
nor these historical ignored tests establishes current FARGAN acceptance.

## Smallest recommended recovery and ownership

| Candidate | Active owner | Recommended single result | Prerequisites and boundaries |
|---|---|---|---|
| 5196 | [5754](https://github.com/dancxjo/conduit/issues/5754), under 4907 | Port v2 scorer compatibility admission and its foreign/invalid-model refusals against the current parser interfaces. | Accepted 5722 artifact and current revision/legal-action contracts; preserve deterministic fallback. Do not import old Session, beam or training machinery. |
| 5273 | [5769](https://github.com/dancxjo/conduit/issues/5769), under 5261 | Port the finite syllable membership/nucleus/stress relation with original-occurrence custody and its negative tests. | Current committed linguistic carrier and selected IPA; preserve Language/Variety/revision. Exclude phoneme-to-phone realization and acoustic timing. |
| 5335 | [4907](https://github.com/dancxjo/conduit/issues/4907), Native preparation prerequisite | Recover only the immutable exact-Type record-access recipe with budget/foreign-Type tests. | Resolve current borrowed-value/validated-extent seam first. Keep prepared-family generator changes separate; no Native readiness granted by a framing cache. |
| 5337 | [4907](https://github.com/dancxjo/conduit/issues/4907) and [4898](https://github.com/dancxjo/conduit/issues/4898), coordinated Native prerequisite | Recover the imported-owner descriptor bridge and complete contract/child validation as one later slice. | Current prepared-family foundation must exist; coordinate the shared generator owner before 5336. Do not grow a second descriptor authority. |
| 5336 | [4898](https://github.com/dancxjo/conduit/issues/4898) | Recover generic model-resource payload reservation and pre-adoption budget refusal with current focused tests. | Current bounded signature and imported descriptor APIs, then the original fixture for its model-specific tests. Keep physical/realtime/listening and full inference memory claims open. |

These are recovery recommendations, not owner handoffs or approval to merge
blocked code. For the Native chain, resolve the foundation, then descriptor
bridge, then model-resource preparation. The active owner chooses one slice
when prerequisites and a review slot exist; [R2 #5663](https://github.com/dancxjo/conduit/issues/5663)
remains open until an actual bounded recovery passes current source/Plan/Play
proof. Original PR discussion and proof remain available through the links.

## Focused review evidence

The inventory was produced without checking out or merging any dormant tree:
full binary diffs were hashed, current blobs compared, and `git merge-tree`
performed with each original PR base explicitly supplied. Its temporary result
trees are review objects only. The working checkout and original heads stayed
unchanged. All five remote recovery heads still matched the original closed,
unmerged PRs after review.

This proves provenance, overlap and conflict classification. It does not run
old bindings, speech, models or hardware. Closing R1 requires this review's
accepted source and inventory verification; it does not close #5388, R2 or the
linguistic/model owners.
