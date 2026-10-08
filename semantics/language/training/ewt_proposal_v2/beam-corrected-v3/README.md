This is a separately pinned TRAIN-only numerical candidate, not a selected runtime
model. It warm-starts the origin-corrected `400b304d…` artifact and changes the
training objective to match fixed four-slot search: update when the gold path is
pruned, or when the complete gold path survives but loses final ranking. Neither
runtime features, legal transitions, beam width, nor Source laws change.

The reproducible one-epoch run uses all 3,530 covered TRAIN rows and the same 22
reviewed teaching rows repeated 32 times, seed 4907, a 512 MiB address-space limit,
and the original i16/score ceilings. Two independent runs produced artifact
`f70f560d54a81dd34d005254985dbfd394363def5f5a4ffe3e97e7c90306d2c5`.
The portable reference retains the exact original feature-function ASTs; only
its import location changes. Full original Source arithmetic parity remains in
the predecessor archive. The complete canonical signature is unchanged, while
artifact and training declaration identities are new.

The numerical reference fits all 22 teaching graphs and POS sequences. Those
are training diagnostics. The separately frozen full DEV/TEST diagnostic retains
all 822/933 rows, includes explicitly identified sequence-disjoint subsets, and
shows substantial remaining errors, especially vocatives. It is not actual
Source/Native execution, a runtime refusal count, stable-fact acceptance, or a
public Session result. No heldout metrics were used to choose this one-epoch
candidate or change its weights.

Run `python3 train.py TRAIN.conllu DICTIONARY_DIR TEACHING.json NEW_OUTPUT 1 32`
from any directory. The full TRAIN digest, dictionary extent, teaching extent,
original artifact digest and numerical limits are checked. Inputs are the
already archived TRAIN dictionary/teaching material and independently pinned UD
TRAIN file. Run `python3 test_train.py` for focused loss regressions. The final
independent artifact/resource packet and actual original Source/model evaluation
remain required before selecting this candidate. Static Final input without a
producer-declared stable prefix still grants no lexical fact or commitment.
