# CI for contributors and agents

Open ordinary pull requests to `dev`. Read the single required `candidate`
result. Its failure identifies the command and source being checked; reproduce
through `cargo xtask ci pipeline`. Actions schedules work; xtask performs it.

The default dispatcher keeps Host, semantic and product packages out of its
transitive dependency graph. The no-option `cargo xtask check acoustic-flow`,
`cargo xtask check acoustic-control`, `cargo xtask check angle-reference`,
`cargo xtask check dsp-shared-compositions` and `cargo xtask check inverse-angle`
entrances execute their same shared suites directly; option-bearing forms retain
the full CLI parser. A regression test checks the actual default Cargo graph,
including transitive dependencies. Focused entrances also compare their compiled
suite definition with the current checkout before executing any step. A stale
dispatcher from a shared build cache refuses instead of running an older proof
subset; rebuild from the current checkout using a separate cache.

## Candidate

The pipeline scans the diff, checks patch hygiene, Rust formatting, locked
workspace metadata and firmware lockfiles, artifact/publication invariants, and
Actions syntax. This `candidate/quick` check runs on every PR update, including drafts.

Draft and ready PRs use the same admission checks. The required `candidate`
result succeeds when all selected checks succeed; draft status itself is not a
failure. GitHub still prevents merging a draft until it is marked ready.
Changing only draft/readiness does not rerun or cancel proof for the same source.
Opening, reopening and new commits still run Candidate against the exact merge
preview; all required checks and branch protection remain unchanged.

PR admission runs Clippy with warnings denied for the selected packages and their
dependencies, then the owning packages' unit, integration and doc tests, with one
test thread. Broad changes retain workspace Clippy; every dev Integration still
runs exhaustive workspace lint and tests. JavaScript pipeline tools, workflow YAML and prose are
validated by preflight without compiling Rust tooling. Mixed edits still select
the changed runtime packages; unknown formats keep their conservative fallback.
Both sides of
renames are scanned against the actual dev parent of the tested merge preview;
GitHub's PR metadata can retain an older base SHA after dev advances. Unknown paths, removed packages, shared files and empty
diffs select all workspace packages. CI scripts and workflow changes run tooling
package tests and the preflight pipeline tests. Recognized prose needs only preflight.
Authored plots and their inventory select the product and catalog tooling consumers.
Numerical-only catalog edits run the full catalog library tests and numerical
Plan/Play integration tests. Numerical registration edits run the full product/Std
library tests and their numerical integration tests, including actual CLI execution.
Any other consumer source/test edit restores its complete package suite; global or
unknown changes disable narrowing entirely.
Numerical libraries and integration targets share one feature-unified compiler
context. Their actual numerical CLI tests provide the relevant Std smoke; they
do not repeat a generic Hello journey. Other architecture and hosted/browser
changes run the Std journey; architecture changes
also run multi-placement and new-Plan recovery. Browser source changes build WASM.
The broader `cargo xtask integrate` loop remains available locally and is not
required for unrelated PR changes.
`cargo xtask check acoustic-flow` exercises the declared four-frame, 8 kHz mono
PCM patch through Source, Plan and the production kernel. It covers timed
control refusal, identical state under producer-block partitions, explicit queue
pressure, closure with pending work, cancellation and a deterministic Host Call
sink that loses its provider. These are finite software proofs, not physical
speaker or device evidence. Admission optimizes test execution while
retaining debug assertions and overflow checks. Reproduce with
`cargo xtask ci pipeline admission BASE_SHA`. Cancelled workflows skip their final
gate jobs so obsolete runs cannot block replacement checks; ordinary failures still
fail the required gate.

Changes to admission, authority, kernel execution, planning, numerical data,
USB, cancellation, certificates or isolation additionally select core, kernel,
planner, assigned-plan, protected-line, composite and data tests, including
alloc-feature kernel conformance. Numerical source/strategy changes also run the
focused catalog numerical Plan/Play tests with the required `kernel-step` feature. ConduitOS
kernel, domain, build and proof changes retain all five ConduitOS target lanes
before merge because workspace tests cannot establish native isolation or device
ownership. This conservative exception includes changes to those proof collectors.
Browser/site changes retain the browser lane, and isolated firmware changes
retain their owning target family. Ordinary package/test corrections do not require unrelated product journeys or
cross-platform packaging.

The `candidate` job requires successful preflight, admission and any selected
native target proof for GitHub’s exact PR merge-preview commit. This checks the
change together with current `dev` and keeps CI tooling available to older PR
branches when the admission policy changes. The preview is a candidate, not a
publication source. Integration remains exhaustive: all
unit shards, workspace lint and every target run after a push to `dev`, even for
prose-only changes. Failed integrated proof must be repaired before release;
publication continues to require those exact verified artifacts. PR admission
never establishes stable acceptance or supplies publication receipts.

Broad candidate admission uses the same complete parallel workspace unit shards
as Integration, including workspace lint. The stable required `admission` check
waits for their aggregate success, then runs the remaining feature-specific
checks and integration smoke. Narrow changes retain affected-package admission.
A skipped, missing, failed or cancelled workspace aggregate cannot satisfy broad
admission. Shard tests also compare their enabled package features with the
workspace-wide feature inventory; the Std Host shard explicitly retains local
model proof and Pete/Create coverage. This changes scheduling, not the required
test corpus.

The selection and execution rules live in `tools/ci/pipeline/admission.mjs`.
The integration matrix remains in `plan.mjs` and `targets.mjs`. Target proof
failures do not cancel sibling lanes. Code tests have no retries; acquisition
may have one bounded infrastructure retry. Browser acceptance retains pinned
Chromium, one worker and zero retries.

Unit and admission jobs retain Rust compiler caches even after a failed check, so a test correction can reuse compiled dependencies. Cargo still validates its dependency fingerprints and every required check runs again. These are build caches, not accepted test results or release evidence; target proof caches retain their existing failure policy.

## On-demand runtime measurements

Runtime measurements are diagnostic and have no calibrated regression threshold.
They run through the **Runtime performance measurements** manual workflow for the
selected ref, or locally through `cargo xtask prove runtime-performance`. They do
not start on every PR or occupy a runner needed for Candidate admission. Failed
measurements retain diagnostics and remain visible as failures in their own run.

## Tool acquisition

Each lane resolves one package set and checks installed tools before acquiring
anything. IA-32 UEFI firmware uses the shared
`cargo xtask make conduitos prepare-ia32-firmware` and
`doctor-ia32-firmware` routes instead of a distro `ovmf-ia32` package dependency.
The [reviewed pin](../../targets/conduitos/tools/ia32-firmware/README.md) records
the signed Ubuntu provenance and package/file hashes. Verified cached pairs
work offline; partial or corrupt pairs refuse. Boot never acquires firmware. Linux lanes use one apt transaction; browser dependencies come from
the pinned Playwright package. Pico radio assets are already versioned source
and are verified without downloading them again. AVR checks its retained core
and compiler identities before repeating setup. Fresh AVR setup installs the
five exact core/tool archives from reviewed official HTTPS URLs and SHA256 pins,
without consulting a mutable package index. Its versioned content receipt binds
those archive identities and the entire installed tree. This archive-pin trust
mode does not claim Arduino package-index signature verification; no CLI
signature check is disabled. An existing unreceipted package tree is preserved
and refused instead of silently adopted. Cargo-installed tools and ESP
compilers retain their exact version and content checks on cache restoration.
The AVR check and Pico doctor use the dependency-light xtask dispatcher and
the same target-owned verification modules as the full tooling, so acquiring
tools does not compile the product workspace.

Acquisition caches are separate from compiler and product artifacts. Their
keys include the runner image, platform, architecture, and checked-in tool
specification. Apt archives additionally bind the installed baseline and exact
resolved dependency versions; cached bytes must match current authenticated
repository metadata. No cache restores `/usr` or the package-manager database.
APT acquisition supports the Ubuntu 24.04/noble and 26.04/resolute amd64
runner profiles. Build prerequisites resolve exclusively from the official
HTTPS Ubuntu archive and security repositories, with main, restricted, universe,
and multiverse in the release, updates, and security suites. The runner's Ubuntu
archive keyring supplies signature verification. Other profiles refuse; a
package requiring a vendor repository is unsupported and fails resolution.

Each acquisition uses private source configuration and fresh indices, leaving
host sources and lists untouched. Unrelated vendor outages cannot affect that
scope. Required archive update failures remain fatal, including on cache hits;
unauthenticated, weak, insecure, and downgraded repositories remain forbidden.
The archive cache identity includes the exact selected sources, signing-key
content, runner image, architecture, requested packages, and installed baseline.
Cached bytes must match current metadata from that same private index scope;
host indices and binary metadata caches cannot authenticate them. Before either
download or installation, an APT simulation must contain no removals and no
version decrease against the recorded baseline, using dpkg's version ordering.
APT may classify a same-version reinstall with different metadata as a downgrade;
that exact version is allowed only after this guard and repository authentication. If a cached
version is no longer authenticated, acquisition refuses; this is not a
historical apt snapshot.

The original vendor outage for [#4984](https://github.com/dancxjo/conduit/issues/4984)
remains in its [failed acquisition job](https://github.com/dancxjo/conduit/actions/runs/37180737785/job/111376354133)
and `acquisition-esp32-c3` artifact. It proves a setup failure before compilation,
not a product execution failure. ESP32-C3 candidate evidence proves acquisition
and compilation separately; it does not prove physical device execution.

Every setup emits `target/acquisition/<lane>.json` with verified identities,
operations, durations, cache outcomes, and known download sizes. The supported
local entrance remains `cargo xtask ci pipeline setup <target>` (`setup-unit`
and `setup-ci` cover portable checks and CI validation).

The **Measure tool acquisition** workflow compares a cold project cache with
an exact restored cache on a second, fresh hosted runner. It runs when a tooling
PR becomes ready for review, or manually with selected target IDs or `all`.
Each target's cold and warm jobs form one bounded pair; at most two pairs run
concurrently. A whole-matrix cold phase can evict its own cache entries before
any warm phase begins when the repository cache is busy.
Its default selection covers apt, validation tools, QEMU, Chromium/npm, Cargo
tools, AVR, and both ESP compiler families. Measurements include cache
restoration and saving, but exclude common checkout and baseline Rust/Node
provisioning. They require matching source, runner image, specification, and
actual tool identities. A slower warm run is retained as a regression rather
than reported as a speedup. The report separately compares warm preparation
against uncached setup alone, so the cost of populating a cache cannot hide
a regression. Existing runner-image tools are part of the
recorded baseline; “cold” does not mean an empty machine.
The warm job restores the cold job's exact cache key. If GitHub assigns a
different runner-image revision to the two jobs, the identity check refuses a
performance comparison; that pair must be measured again on matching images.

The [6 October 2026 acquisition run](https://github.com/dancxjo/conduit/actions/runs/37441249185)
completed exact cold/warm pairs for every target below at source
`b4b2157910727c94bca6e7a02cf8358edcffe0de`. These are preparation
times in seconds, including project-cache restore and save but excluding
checkout, baseline runner provisioning, product build, and product proof.
They measure that source and runner image, not a promise for later images.

| Target | Cold | Warm | Saved |
| --- | ---: | ---: | ---: |
| Unit | 31.2 | 25.5 | 5.7 |
| Preflight | 1.7 | 0.6 | 1.1 |
| Browser | 54.4 | 41.3 | 13.1 |
| Hosted Linux | 37.5 | 25.0 | 12.4 |
| Hosted Windows | 2.7 | 1.7 | 1.1 |
| Hosted macOS | 2.2 | 0.7 | 1.5 |
| ConduitOS x86_64 | 63.9 | 56.2 | 7.8 |
| ConduitOS AArch64 | 43.2 | 38.8 | 4.4 |
| ConduitOS IA-32 | 61.3 | 43.6 | 17.7 |
| ConduitOS RISC-V64 | 51.7 | 45.0 | 6.7 |
| ConduitOS LoongArch64 | 73.0 | 60.3 | 12.7 |
| ESP32-C3 | 55.2 | 30.2 | 25.0 |
| ESP32-S3 | 220.0 | 34.6 | 185.4 |
| ESP32-WROOM | 290.8 | 35.6 | 255.2 |
| AVR | 87.1 | 39.6 | 47.5 |
| Raspberry Pi | 39.7 | 29.4 | 10.3 |
| Orange Pi | 44.4 | 39.7 | 4.8 |
| RP2040 | 42.3 | 27.0 | 15.4 |

IA-32's cold setup spent 51.7 seconds in the one APT transaction; the warm
setup still spent 35.9 seconds updating private signed indices and installing
authenticated cached packages. Rust target/component acquisition took about
seven and six seconds respectively. The earlier 40-minute job therefore
cannot be explained by IA-32 tool setup alone. Windows, macOS, and preflight
warm restores cost more than uncached setup when cache restore overhead is
included, so the pipeline does not retain acquisition caches for those lanes.

## Combined development

Every push to `dev` runs all supported targets. Running integration finishes;
newer pending updates coalesce. The full target list is explicit in
`tools/ci/pipeline/targets.mjs`; adding a target changes both scheduling and the
required publication set. There is no second product build campaign.

Unit jobs retain finite per-shard wall-clock limits: foundation has 90 minutes,
products and the ordinary Host groups have 70, ConduitOS has 120, and lint has
40. The cold foundation path includes its main tests, doctests and a separate
kernel allocation-feature pass; products includes browser Owner proof. Foundation,
products, Std Host, browser Host and ConduitOS tests use optimization level 1 with debug assertions and
overflow checks enabled. The same tests and features remain selected; this profile
changes execution cost, not the required proof set.
These limits include compilation and preparation. They do not permit test
retries or reduce the required proof set.

The browser target has a finite 90-minute whole-job limit, including cold Host
and WASM builds, the one-worker/zero-retry Chromium suite, site proof and artifact
retention. In [the 19 October cold candidate](https://github.com/dancxjo/conduit/actions/runs/39212641215/job/117930615050),
preparation took about 28 minutes and the suite reached test 258 of 260 before
the former 60-minute job limit cancelled it. No assertion failure was logged
before cancellation; that partial run is not acceptance. Other target job limits
remain 60 minutes. Per-test preparation/response limits and all required proof
remain unchanged.


A lane writes a receipt only after its build and proof succeed. The receipt
records its source commit, target, proof class, and complete product file hashes.
Products leave the runner in tar archives so executable permissions survive
artifact transfer. Failed lanes can retain diagnostics, but cannot issue a
successful product receipt. GitHub's **Re-run failed jobs** keeps successful
siblings; test failures are never automatically retried.

Build proof for firmware and boards without CI hardware remains **build proof
only**. Executable smoke, browser execution, and ConduitOS emulator boot are
separate classes. None is physical or human acceptance. Browser coverage names
its actual executed specs; cross-device enactment remains separate work.
LoongArch uses Ubuntu 26.04 for QEMU 10 or newer; the verifier refuses older
emulators before boot because their large-page translation can corrupt the
bootloader's module handoff.

The earlier foundation bound increased from 40 to 60 minutes after a cold run
passed its reported assertions but exhausted the job during doc-tests. The
current 90-minute foundation limit above supersedes that checkpoint.

The x86_64 lane exercises the Face journey against the same retained product
image: zero-Body Crèche arrival, Birth into rest, Wake, Plan, Play, Home,
Patchbay, Face inspection, a live graph diagram, and Stop. The command
`cargo xtask make host verify OUTPUT --journey` verifies the digest before and after the
QMP-driven keyboard session and correlates the guest's source, profile, build,
image, Boot, and Body identities. It does not rebuild a demonstration image.
The lane retains ten actual screen captures, their manifest, and the journey
receipt with the product. Cold product arrival has a bounded 20-second preparation
budget for source-catalog admission and native Mask planning. Subsequent HID
response checks retain their separate bounds. The prior five-second input-response
budget left little margin for cold preparation: a local integrated native boot
reached Crèche at 3.6 seconds, while the hosted proof stopped after identity
admission at its five-second cutoff. This emulator journey does not establish physical
hardware operation, screen-free use, or human enactment.

## Publication

Only a successful, repository-owned `push` integration run can publish. The
publisher independently checks the run identity and every required artifact;
missing targets, wrong source commits, altered files, unknown targets, and
mismatched proof classes refuse publication. PR artifacts cannot enter this
path. A manually dispatched integration is diagnostic and does not auto-release.

Publication creates `release/<tested-dev-sha>` and promotes through a PR to
`main`. Another open release or release synchronization blocks a successor.
Accepted `main` must be an ancestor of the tested source, or a prior release
merge with an identical tree to its source parent, which is an ancestor of the
new source. This avoids empty synchronization commits and their duplicate CI.
The merge is guarded
by the release head, and its resulting tree must equal the tested tree before
any release assets are published. The release manifest records the tested
source SHA and accepted main merge SHA separately; a merge commit is not
misrepresented as the original tested commit.

The publisher packages the verified products as GitHub Release assets. It does
not run unit tests, rebuild products, or start another CI campaign. A rerun
validates existing publication before resuming; it never silently replaces a
published asset. Publication creates and verifies the exact release tag before
creating a release, so a later workflow change on dev does not require granting
the publisher permission to edit workflows. GitHub retains the complete run and
check identities.

The browser lane assembles and checks the public website, including the tested
Workspace and fresh Field Station Clock screenshots. Its sealed receipt includes
the site and desktop/mobile browser checks. After restoring the verified product
bundles, publication copies that site and adds the x86_64 journey pages from the
retained QMP frames. The composition validates the exact source and screenshot
hashes; it never changes the sealed browser bundle. The derived site goes to
GitHub Pages only after release acceptance, and an older release cannot replace
the current site. No product or website build runs during publication.

Older Three Bodies and technical recordings retain their original identities,
media, and explicit capture limits. Updating their presentation does not turn
them into execution proof for the new release. The website publication manifest
records current source and retained history separately.

The One Body, five Masks page appears only when
`site/evidence/one-body-five-masks/` contains one complete producer-owned
manifest and all its declared outputs. Capture runs outside hosted CI where
the browser, QEMU guest, local model, and speech device can actually operate.
The captured source commit must be an ancestor of the later publication
commit. Preflight checks that relationship, the browser lane renders and
checks the exact media and common navigation, and the normal protected release
train deploys the verified site. A diagnostic or partial run cannot create the
complete eight-chapter page. `site-publication.json` records the capture source
and publication source separately; neither source identity implies human
listening or physical hardware proof.

A separately labeled development recording may be retained at
`site/evidence/three-host-development/` while the eight-chapter acceptance
journey is still incomplete. Its `diagnostic-incomplete` manifest declares the
actual page, report, screenshots, and listener WAV files from one local run.
Preflight checks capture-source ancestry; the site lane verifies every retained
byte and copies the recording without running QEMU, a speaker, or a model.
The gallery and publication receipt identify it as partial evidence. The
complete journey and development recording cannot claim the same route in one
publication. This carrier does not turn a partial run into #4807 acceptance.

## Operation and validation

The workflows are `candidate.yml`, `integration.yml`, `publish.yml`, and the
shared `ci.yml`. There are no approval monitors, scheduled reconcilers, dead-man
pollers, artifact retry wrappers, or controller-to-controller wake-ups.
Publication uses one concurrency group and repository branch protection remains
in force. Development requires `candidate`; main requires `release-verification`
after complete artifact verification. Both required statuses are bound to the
GitHub Actions app. Main force pushes are disabled. The repository allows
Actions to create pull requests.

```sh
cargo xtask ci pipeline setup-ci
cargo xtask ci pipeline preflight BASE_SHA
cargo xtask ci pipeline unit foundation
cargo xtask ci pipeline scan BASE_SHA HEAD_SHA
cargo xtask ci pipeline setup conduitos-x86_64
cargo xtask ci pipeline target conduitos-x86_64 HEAD_SHA
cargo xtask ci pipeline verify target/bundle HEAD_SHA
```

`setup-ci` installs checksum-verified actionlint; target setup installs that
lane's prerequisites. Missing tools or unavailable physical devices are not
successful proof. Artifact retention is fourteen days; expired input requires a
fresh integration, not fabricated receipts. Behavioral tests cover refusal and
identity invariants rather than fixing YAML job names or ordering strings.

## Repair an admitted red development source

Capture the failing integration SHA, owner job and retained stderr/stdout first.
A green Candidate or another owner's successful job cannot stand in for full
integration. Compare current dev with accepted main, including the accumulated
batch; an existing release retains its ownership through repair.

Hold unrelated owned feature auto-merges while investigating. Keep their
branches, review discussion and proof; park a PR if needed to keep the active
handoff small. Reproduce the actual owner failure and send the smallest repair
or revert through an ordinary dev PR. Preserve mandatory gates and negative
cases rather than treating failure as an optional status.

For example, hosted retention failed after the Face migration when the correct
hello payload acquired the display label `Greeting:`. The repair validates the
exact emitted Face bytes independently of decoration; host, completed Plan,
runtime report and unchanged-executable checks remain required. Tests reject a
friendly caption with wrong bytes, missing completion or a foreign host.

After merge, require exhaustive integration of the repaired exact source and
verified publication of those same artifacts and accepted tree. Retain run,
job, source and publication identities, and report actual lag while pending.
Resume held feature admission only after that restoration is green. Never
substitute an older green status, bypass protection or claim stable acceptance
from Candidate alone.

## Observe delivery by exact source

Run `cargo xtask ci pipeline status dancxjo/conduit` with authenticated `gh`
access. The read-only JSON report extends the existing product-truth identity
separation using Actions observations and the publisher's existing
`conduit.pipeline-release/v1` manifest. It shows admitted `dev`, the integration
for that exact source, the latest observed green integration, accepted `main`,
the latest published release's tested source and accepted main, and the latest
successful Pages deployment's source. Differences are explicit; a green
integration alone never asserts promotion or deployment. Candidate
`head_sha` is labeled as the PR head, separately from its tested merge preview.
The latter remains `null` until checkout/receipt evidence is observed; a green
PR-head run must not be presented as proof of an inferred preview identity.

Failed and pending integration/publication, the latest Pages attempt, and active
queues remain visible. A missing observation is `null`, not success. The report
is bounded to the latest 100 runs per workflow, five candidate job selections,
20 releases and 20 Pages deployments; it is an observation, not release state.
API failure refuses the report instead of inventing evidence. Save the JSON
before changing delivery policy to retain the wall-clock timing baseline and
exact run URLs. Durations use Actions `run_started_at` through `updated_at`,
include setup, exclude the pre-start queue, and are not billed runner cost. No
delivery SLO thresholds are introduced. Publication still verifies and reuses
the tested artifacts through its existing owner.

The status observer orders release manifests by their explicit publication date.
GitHub's release-list order is not release recency. Publication runs and Pages
deployment records expose workflow-head identities, which can differ from the
built source. `live_pages_publication` reads the existing publisher-owned
`site-publication.json` from the configured Pages URL; only that observed
manifest supplies the current deployed build source. Unobserved deployment
record source stays null, and failed observations refuse the report.

