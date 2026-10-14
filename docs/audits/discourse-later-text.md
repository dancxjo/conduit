# Later-text discourse revision proof

For #5723, use the existing checked vocative rule and shared revision ledger.
No punctuation classifier, parser, discourse runtime or pause policy is added.

The existing three-placement fixture derives addressee links from exact supplied
UD vocative arcs in sentence-initial, medial and final positions. Nonvocative
relations refuse even when text contains punctuation; wrong source/analysis and
out-of-range occurrences refuse before derivation.

The new fixture begins with partial `hello Travis` Source at revision r1 and an
admitted vocative edge. Later `hello Travis arrived` Source at r2 retains the
previous immutable material, exact prior/sequence and an admitted text-revision
lineage. Its supplied analysis a2 instead contains an nsubj edge. That edge cannot
manufacture a vocative fact. A ledger withdrawal naming the original r1 occurrence
and retaining r2/a2 evidence succeeds while the original inference is uncommitted.
The same withdrawal refuses after the original region is committed. The committed
frontier/history and canonical original fact bytes remain unchanged.

This proves a deterministic explicit retraction policy from supplied canonical
syntax and later Source custody. It does not claim a parser automatically found
those arcs or that string spelling/punctuation established syntax. No real-person
identity, speech timing or playback authority is inferred. The old fact keeps its
original Source rather than being relabeled with the later revision.

Fifteen focused tests pass across discourse, text_revision and revision_lineage.
Strict Clippy with warnings denied passes. Merge acceptance remains pending. Parent #4907 retains its full
end-to-end language/speech/multimodal requirements.
