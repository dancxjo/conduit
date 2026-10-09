# Bounded speech selection

Run `cargo xtask check todo-speech` to check direct speech policy against the
actual Todo-owned Face projection and reusable Face fixtures.

The direct spoken Mask and interactive `summary` use one deterministic
`SpokenOutline`: exact Face identity/revision, selected text indices and action
identities, and bounded human clauses. Collection/document title and authored
Context lead; primary status and at most three primary items follow. An available
current content action is selected when space permits. Completed-item detail,
unknown properties, diagnostic facts and unavailable actions remain inspectable
through the canonical Face and `read all`.

In the screen-free reader, `read remaining` begins a page of up to three primary
items; `more items` advances it. The cursor carries Face correlation and an
ordinal, not application state. A changed Face refuses continuation until a new
selection is requested. Pending audio refuses advancement. `read current items`
retains the existing complete primary-item stream; `read all` includes detail.
For Todo, the Plot marks remaining items primary and completed items as detail.
The speech policy does not infer completion from names or mutate the list.

Finite model wording uses grammar variants from the same selected text and
available action references. The Host checks selection, exact Face values and
revision before accepting speech; raw provider output and refused outcomes
remain retained. Properties and unselected actions cannot be smuggled into the
primary opening through model wording. This is a finite wording policy, not a
claim that arbitrary model paraphrases can be semantically verified.

The suite covers three open/seventeen completed, a full list's unavailable Add,
empty/all-completed lists, Unicode/punctuation, unknown fields, twenty remaining
items over seven pages, stale Face identity/revision, audio pressure, and model
fabrication refusals. Existing ticker presentation uses this same policy;
#5269's interval-ticker wording is a separate change.

These deterministic checks prove policy and input behavior. They do not prove
acoustic quality, speaker playback or human perception. The existing eSpeak
provider can be selected explicitly using the runtime speech workflow; its
executable, library and voice data remain separate admitted resources. This
change records no new audio demonstration. The same-Face spoken capture remains
part of cross-Mask journey acceptance, separate from this selection contract.

The coordinated direct-readout and paging implementation at
`54e8d77d45742d97486197aa8afc34772535d485` passed all 26 jobs in
[Candidate 37881958506](https://github.com/dancxjo/conduit/actions/runs/37881958506).
That campaign includes the browser cross-Mask journey and the installed product
proofs. It is evidence for that exact commit; a later revision still requires
its own successful Candidate before admission.
