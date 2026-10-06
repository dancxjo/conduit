# CI for contributors and agents

Open ordinary pull requests to `dev`. Read the single required `candidate`
result. Its failure identifies the command and source being checked; reproduce
through `cargo xtask ci pipeline`. Actions schedules work; xtask performs it.

## Candidate

The pipeline scans the diff, checks patch hygiene, Rust formatting, locked
workspace metadata and firmware lockfiles, artifact/publication invariants, and
Actions syntax. This `candidate/quick` check runs on every PR update, including
drafts. Draft PRs stop after quick checks; mark a PR ready for review to launch
exhaustive proof for its current SHA. Returning it to draft cancels superseded
proof and runs quick checks again. Keep actively changing work in draft.

After preflight, unit and target proof start independently. Foundation and
products retain their broad unit shards; hosts split into std providers,
browser runtime, ConduitOS, and workbench fixtures. Workspace Clippy runs
alongside them. Ordinary packages belong to exactly one group. The sustained
ConduitOS proof is partitioned across the same runners: automatic Body cases
with std, automatic Clock cases and HID reports with browser, USB protocol plots
with workbench,
and remaining library, default integration, and doc tests with ConduitOS.
The native lane validates the compiled library inventory; Cargo metadata assigns
every default integration target. Each group retains `--test-threads=1`.
Isolated browser assets skip these native slices; Integration always proves them.
A unit failure does not prevent independent targets from reporting defects.
Reproduce a host group with `cargo xtask ci pipeline unit hosts-std` (or
`hosts-browser`, `hosts-conduitos`, `hosts-workbench`). The full xtask check
entrance also exposes `workspace-test-hosts-std` and the other named groups;
`workspace-test-hosts` retains the aggregate local suite.

The `candidate` job is an AND gate over all selected proof for the exact PR SHA.
It explicitly blocks admission on drafts while quick checks can pass; a skipped
required check would count as successful in GitHub branch protection. Ready docs-only
PRs require successful preflight. Integration always runs exhaustive proof for
all targets and unit shards, with running work finishing and pending commits
coalescing. Publication continues to consume those exact verified artifacts.

Selection is deliberately small and conservative:

| Change | Target families |
| --- | --- |
| Prose documentation only | Preflight |
| Browser, browser proof, site | Browser |
| Isolated target firmware | Its family |
| Hosted source | Hosted and browser |
| Shared target runtime, offers, or make libraries | All |
| ConduitOS | ConduitOS and Orange Pi |
| Shared code, manifests, tools, workflows, unknown paths | All |

Unit selection is conservative too: isolated browser proof scripts and site
assets select browser and workbench host fixtures, products/tooling tests, and
workspace Clippy. Other source changes, dependency files, unknown paths, and
mixed changes retain every unit shard. Integration always selects all shards.

Both names of a rename are included. An empty diff selects all. There are no
receipt-reuse fingerprints or path dependency controllers. The selection rules
live in `tools/ci/pipeline/plan.mjs`.

Every selected target owns setup, build, proof, packaging, and one final
artifact upload on its runner. Targets never depend on an unrelated target.
Compiler caches retain Cargo compiler directories and dependencies. Lanes with
measured acquisition savings, including browser, Linux, ConduitOS, AVR, ESP32,
and RP2040 targets, use a separate cache for verified tool downloads and
pinned installations. Preflight, Windows, and macOS acquire their tool sets
directly because their measured warm restores did not save time. Staged
products and product proof always start fresh.
Once the target matrix starts, a failure does not cancel siblings. Code tests
have no retries. Acquisition may have one bounded infrastructure retry.
Browser acceptance keeps pinned Chromium, one worker, and zero retries.

## Tool acquisition

Each lane resolves one package set and checks installed tools before acquiring
anything. Linux lanes use one apt transaction; browser dependencies come from
the pinned Playwright package. Pico radio assets are already versioned source
and are verified without downloading them again. AVR checks its retained core
and compiler identities before repeating setup. Cargo-installed tools and ESP
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

The x86_64 lane exercises the Face journey against the same retained product
image: zero-Body Crèche arrival, Birth into rest, Wake, Plan, Play, Home,
Patchbay, Face inspection, a live graph diagram, and Stop. The command
`cargo xtask make host verify OUTPUT --journey` verifies the digest before and after the
QMP-driven keyboard session and correlates the guest's source, profile, build,
image, Boot, and Body identities. It does not rebuild a demonstration image.
The lane retains ten actual screen captures, their manifest, and the journey
receipt with the product. This emulator journey does not establish physical
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
train deploys the verified site. A diagnostic or partial run does not create
the page. `site-publication.json` records the capture source and publication
source separately; neither source identity implies human listening or physical
hardware proof.

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
