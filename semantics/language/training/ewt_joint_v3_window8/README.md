# Window8 training preparation

This is a distinct eight-token profile with root8/unassigned9,425 categorical
features,76 arc-eager action labels and25 lookups. Four-token/v2 artifacts and
Source definitions remain unchanged.

`reviewed_teaching.json` contains separately authored TRAIN-only supervision for
noun/verb `record` and `refuse`, a later-context garden-path interpretation,
complement, coordination, apposition, parenthetical and question structures.
The exact `I record the record.` surface includes its punctuation token.
These graphs are reference supervision, not predictions or held-out accuracy.

`reviewed_lexical_alternatives.json` supplies finite lexical policy data. Model
preparation reserves those exact-case forms and fills the remaining64-entry
native profile capacity using eligible TRAIN frequency. DEV/TEST do not choose
profile vocabulary or alternatives. Unknown complete forms and over-eight
complete prefixes refuse rich admission; a trailing partial word stays separate.

The training-only Python reference uses the same declared425/76/25 arithmetic,
but its states/actions are untrusted. Actual Source forest/mutation/feature
admission and native planned model decoding are separate required gates.
No trained artifact or native learned accuracy is established by these files.

Pinned read-only corpus: official UD_English-EWT2.18 commit
`b7711cce01cdd4f5fcc0a8199b8a50d951b16c0c`, CC BY-SA4.0. The training script
records each supplied split digest, exclusions, covered counts, artifact and
Source contract identities, and labels gold-state diagnostics explicitly.

`vocative_training.json` is a separate TRAIN-only extension: the original eight
teaching cases plus greeting, initial, medial and final vocatives.
`vocative_lexical_alternatives.json` explicitly adds comma, `Hello` and `Thanks`.
These are authored reference supervision, not human-reviewed gold or evaluation
examples. The Python oracle can represent all four new reference trees; that
check does not establish Native decoding, stable facts, prosody or speech.

The original eight-case artifacts remain pinned. A fresh candidate can select
these inputs explicitly with `train_ewt_window8.py --teaching PATH
--lexical-alternatives PATH`; the manifest retains their exact content digests.
Do not relabel a TRAIN replay as held-out performance. Actual Source decoding,
independent evaluation and three-placement speech acceptance remain required.
The new preset changes linguistic parser supervision only; it does not advance
an acoustic voice model or satisfy the canonical IPA/shared-intent #5212 gate.
