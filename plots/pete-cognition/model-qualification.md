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

Every Gemma4 scenario completed through its existing ordinary kernel path and
produced nontruncated model results. The bodily sequence retained one Body/Wake/Play and
distinct model runs. Independent test harnesses use declared fixture Host/Boot
identities; the Cargo test names identify separate scenarios, and their native
Plan/Play/run identities must not be treated as global real-Host uniqueness.

These failures leave this candidate unqualified for the complete #6874 envelope.
The assertions, defaults, authored instructions and resource bounds remain
unchanged. The result does not establish a preferred replacement configuration
or close the owning issue. The first-run logs and configuration receipts remain
in ignored local proof output; required durable publication is separate work.


## Comparison with the installed Phi4 candidate

The next candidate used the identical four cases and assertions: `phi4:latest`,
Ollama 0.34.1, 12288 MiB admitted memory, and inventory digest
`ac896e5b8b34a1f4efa7b14d7520725140d5512484457fab45d2a4ea14c69dba`.
The first result for every case was retained. No prompts or resource bounds
changed to accommodate its answers.

| Instruction envelope and case | Result | Observed behavior |
| --- | --- | --- |
| Pete: direct human report | Quality failure | Answered “I do not know.” despite the supplied current human report of 72%. |
| Pete: previous/current/unavailable battery | Passed | Recalled 50.0%, answered current 9.0%, then answered “I do not know.” when the current source became unavailable. |
| Generic explicit evidence: contradictory reports | Bounded-output failure | Returned `Truncated` after discussing only the first label; no complete answer preserving both alternatives was delivered. |
| Generic explicit evidence: uncertain human report | Bounded-output failure | The partial text mentioned the uncertain human-report classification, but the native result was `Truncated`, so it did not qualify as a completed answer. |

Initialization succeeded for all four cases. Native truncation remains separate
from a completed answer failing its grounding assertions and from provider or
transport loss. Increasing a budget, changing the instructions or accepting the
partial text would change this comparison; none was done. Neither candidate
passed all four new cases, so this campaign did not advance either to an
additional run of the existing narrow recall/current/empty-history scenario.

The already installed `gpt-oss:20b` artifact was initially considered but not invoked.
Its preflight measured 24,417,243,136 bytes available; a 20 GiB admission would
leave 2,942,406,656 bytes. That did not meet the selected extra-headroom margin
of 7 GiB (one quarter of the admission plus 2 GiB). The candidate was skipped
without downloading models, evicting another model or changing its requested
admission. The ignored preflight record retains the installed artifact digest,
resident model inventory and measured memory.


## Llama3.2 and the later GPT-OSS initialization attempt

The identical cases next used `llama3.2:latest`, Ollama 0.34.1, 4096 MiB
admitted memory and inventory digest
`a80c4f17acd55265feec403c7aef86be0c25983ab279d83f3bcd3abbcb5b8b72`.
Fresh available memory was 35,523,878,912 bytes and the resident inventory was
empty after natural cache expiry. Its first results were:

- Direct human report: passed with 72%.
- Bodily timeline: correctly recalled 50.0% and answered current 9.0%, then
  failed by answering remembered 50.0% when current charge was unavailable/null.
- Generic contradictory and uncertain evidence: both returned `Truncated`.
  Their partial text acknowledged the respective conflict or uncertainty, but
  neither was accepted as a completed answer.

No source, prompt, assertion or resource bound changed for this comparison.
The result did not qualify this candidate.

A later GPT-OSS preflight measured 32,761,171,968 bytes available, exceeding
the same 20 GiB admission plus 7 GiB extra-headroom threshold. The installed
`gpt-oss:20b` inventory digest was
`17052f91a42e97930aa6e28a6c6c06a983e6a58dbb00434885a0cf5313e376f7`.
Only the related Llama3.2 model was resident. This new resource check did not
retry an inference: the earlier insufficient-headroom record remains intact.
No model was downloaded or manually evicted.

The first GPT-OSS case then stopped during the existing bounded initialization
probe with `local model warmup produced no output`. No scenario response was
produced. This is an initialization refusal, distinct from a completed answer's
quality failure or native truncation. Initialization was not retried, and its
remaining cases were not attempted.

None of the evaluated candidates passed all four new cases. The additional
unchanged narrow recall/current/empty-history and fault-injected provider-loss
with live recovery scenarios therefore remained behind their qualification
gate in this comparison. No default was promoted.


## Generic source and certainty instruction profile v2

Issue [#7026](https://github.com/dancxjo/conduit/issues/7026) changes only the
default generic explicit-evidence formatter instruction. The 227-byte
`generic/source-certainty-evidence@2` instruction explicitly asks the model to distinguish human
reports, recall and inference from current observations, preserve certainty and
acknowledge uncertainty. It is five bytes shorter than the
previous instruction. Authored/contextual instructions, omitted-instruction
rendering, defaults and all model/request/output/resource bounds are unchanged.

Receipts include the exact instruction, UTF-8 length, SHA-256 and compiled
formatter source SHA-256. The instruction hash is
`e185f176b61d281bf15a8ca5dd835680f1ce8ae13401d5a527d46501e2b77e13`.
The deterministic tests preserve human-report/uncertain classifications and the
explicit contradiction relationship in actual model-bound requests, beside the
existing two-revision provenance and oversized-render refusal checks.

The first sequential run on 23 November 2026 used Ollama 0.34.1, the same
pinned Gemma4 digest recorded above and 12288 MiB admitted memory. It ran from
the designated original checkout after deterministic checks and warnings-denied
Clippy passed. Each live case ran once; neither response was retried. Both
results were `Produced`, with no truncation:

| Case | Result | Observed behavior |
| --- | --- | --- |
| Uncertain human evidence | Quality failure | Reported COPPER KITE and explicitly identified a human report, but did not acknowledge uncertainty. |
| Contradictory human evidence | Passed | Preserved COPPER KITE and SILVER MOTH as human reports and explicitly said they contradict each other. |

The uncertainty response was: “The crate label is reported to be "COPPER KITE."
This information is a human report.” Correct source attribution does not satisfy
the retained uncertainty assertion. This candidate remains unqualified for that
case; the instruction change does not justify accepting the answer or promoting
a default. The three deterministic tests passed, with three existing opt-in live
tests ignored in the ordinary target. The separate older generic live test still
hardcodes Gemma3 and was not included in this Gemma4 campaign.

The retained original failure remains part of the historical comparison.
This bounded synthetic-evidence proof does not establish universal correctness,
the configured Gemma3 default, Pete's separate authored instruction, physical
robot behavior or stable-release acceptance. The broader #6874 qualification
remains open.
