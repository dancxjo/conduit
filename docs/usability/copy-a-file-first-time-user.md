# Copy a file first-time-user proof

Status: automated browser-safe and hosted implementation evidence complete;
unfamiliar-person observation not yet performed.

This record keeps automated conformance separate from human usability evidence.
The current `library.bounded-filesystem` Tour project opens in Use, requires
separate protected From and To choices and grants, dispatches Copy through the
exact runtime action protocol, and presents the typed `fs/write-result`
separately from terminal state. Its deterministic browser fixture and hosted
local provider prove the implementation path; neither proves that an
unfamiliar person can use it.

## Current implementation evidence

- Rust tests cover missing choices, separate grants, same-resource rejection,
  exact action admission and identity, duplicate requests, cancellation,
  semantic result versus terminal state, runtime failure, and stale epochs.
- Browser source selection uses an actual file input. Selected bytes remain
  worker-memory-only behind the opaque protected binding; neither the bytes nor
  its resource or grant identity enter shared source or task-front projection.
  Destination selection is a separate explicit Replace-and-download ceremony,
  and the resulting download is released only for the exact accepted request,
  run, plan identity, and plan epoch that committed it.
- Chromium, Firefox, and WebKit tests cover the actual browser file-input and
  download interaction, recognizable bounded failure and success with the
  console closed, Build and Inside disclosure, return to Use, keyboard
  operation, an ordinary viewport, and 200 percent zoom.
- The `conduct` runnability inventory executes `examples/file-copier.panel`
  through the production hosted read/write providers and compares the written
  destination bytes with the installed source fixture.

## Open blocking evidence

- [#327](https://github.com/dancxjo/conduit/issues/327) tracks the required
  unfamiliar-person study. Until an observation is recorded there, no human
  usability claim has been established.

## Study protocol

Give the participant the Copy a file project without explaining Conduit or its
language. Record the environment and whether they can state the purpose,
choose From and To, predict Replace behavior, run Copy, identify success or a
precise incomplete/failure outcome, reveal how it works, and return to Use
without losing valid choices. Record every missed step as a product defect and
link its follow-up issue before repeating the study.
