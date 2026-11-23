# Pete model qualification

Issue [#6874](https://github.com/dancxjo/conduit/issues/6874) owns selection of a
reproducible default and its supported quality envelope. These opt-in tests add
bounded cases to the existing hosted proof harnesses. They do not change the
configured default, authored Plots, model request bounds or runtime behavior.

All new inputs are synthetic. The bodily values are lowered from the existing
small Create protocol fixture or supplied as an explicit unavailable source;
no serial device opens. Ollama inference is real. Passing these tests therefore
establishes hosted model behavior for those supplied values, not physical robot
observation, actuation, human listening or universal model correctness.

## Separate instruction envelopes

| Case | Source and instruction envelope | Required result |
| --- | --- | --- |
| Previous/current/unavailable battery | Unchanged `pete-cognition` Plot and its authored persona | Recall the retained 50.0% reading; answer a later current-state question with 9.0%; acknowledge unavailable current charge without recycling either percentage. |
| Direct human report | Unchanged `pete-cognition` Plot and its authored persona | Ground the answer in a human-reported 72%, while the independent synthetic current battery observation remains 9.0%. |
| Uncertain human evidence | Unchanged `experience-response` Plot and the existing generic explicit-evidence formatter instruction | Mention the supplied label while acknowledging its uncertain classification. |
| Contradictory human evidence | The same generic explicit-evidence envelope | Preserve both conflicting label alternatives and acknowledge the contradiction instead of selecting one as certain. |

The generic formatter explicitly asks the model to acknowledge conflicts and
unknowns and to distinguish remembered/inferred claims from current observations.
A passing generic case does **not** establish the same behavior under Pete's
different authored persona. The receipt records that distinction and the exact
instruction used. Existing recalled-label, empty-history, percentage, provider
loss and provenance assertions remain unchanged.

The deterministic companion tests inspect the actual model-bound requests:
source origins and certainty, current and remembered values, unavailable state,
and the explicit `contradicts` relationship must survive the ordinary installed
kernel. A fenced fake proves that plumbing; its answers are not a live-model
quality result. Live tests require completed, nontruncated results, preserve
Body/Wake identity across the bodily episodes, and retain distinct model run
identities and the Plan/Play identities.

## Proof entrances and records

The new modules are registered by the existing `experience_episode_loop` and
`experience_response` integration targets and participate in the existing
repository entrances:

```sh
cargo xtask check pete-cognition
cargo xtask check pete-cognition-live
```

The live suite retains its other tests and their installed-model prerequisites.
Its new cases reuse the existing typed-frame selection helper: the internal
`CONDUIT_PETE_LIVE_MODEL` and `CONDUIT_PETE_LIVE_MEMORY_MIB` settings select an
already installed realization; defaults remain `gemma3:latest` and 6144 MiB.
The new qualification receipt can additionally check
`CONDUIT_PETE_LIVE_MODEL_DIGEST` before the scenario calls. These are proof
settings, not a new authored provider choice or a change to the ordinary
[operator configuration](README.md).

Run selected live scenarios sequentially and retain the first result. Do not
retry a failed answer until a candidate appears green. The developer's targeted
filters isolate these opt-in cases; the public suite continues to include its
existing assertions and prerequisites.

Each live case prints `CONDUIT_MODEL_QUALIFICATION_EVIDENCE` with the initialized
model identity, digest, admitted resource/work envelope, source text/checksum
and instruction profile. Scenario records retain the supplied evidence and
actual model words, run identity and implementation identity before quality
assertions execute. The digest is the adapter's discovered inventory identity;
this does not prove immutable provider-tag custody. Raw logs belong in ignored
`target/pete-model-qualification/` output, with compact results and any required
durable custody tracked separately.

## Recorded candidate outcome

The first sequential local run on 22 November 2026 used Ollama 0.34.1,
`gemma4:latest` with inventory digest
`c6eb396dbd5992bbe3f5cdb947e8bbc0ee413d7c17e2beaae69f5d569cf982eb`,
and 12288 MiB admitted memory. No answer was retried. The three deterministic
plumbing cases passed; the four live quality cases produced two passes and
two failures:

| Instruction envelope and case | Result | Observed behavior |
| --- | --- | --- |
| Pete: direct human report | Passed | Answered 72% from the human report while synthetic observed charge was 9.0%. |
| Pete: previous/current/unavailable battery | Failed | Correctly recalled 50.0% and answered current 9.0%; after the current source became unavailable and current charge was null, answered the remembered 50.0% again. |
| Generic explicit evidence: contradictory reports | Passed | Preserved both label alternatives and explicitly said the reports contradict each other. |
| Generic explicit evidence: uncertain human report | Failed | Said the label was “observed to be” COPPER KITE, with no acknowledgment of the uncertain human-report classification. |

Every scenario completed through its existing ordinary kernel path and produced
nontruncated model results. The bodily sequence retained one Body/Wake/Play and
distinct model runs. Independent test harnesses use declared fixture Host/Boot
identities; the Cargo test names identify separate scenarios, and their native
Plan/Play/run identities must not be treated as global real-Host uniqueness.

These failures leave this candidate unqualified for the complete #6874 envelope.
The assertions, defaults, authored instructions and resource bounds remain
unchanged. The result does not establish a preferred replacement configuration
or close the owning issue. The first-run logs and configuration receipts remain
in ignored local proof output; required durable publication is separate work.
