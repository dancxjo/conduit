# CI for contributors and agents

Open ordinary pull requests to `dev`. Read the single required `candidate`
result. Its failure identifies the command and source being checked; reproduce
through `cargo xtask ci pipeline`. Actions schedules work; xtask performs it.

## Candidate

The pipeline scans the diff, checks patch hygiene, Rust formatting, locked
workspace metadata and firmware lockfiles, artifact/publication invariants, and
Actions syntax. Three
broad unit shards cover foundation, hosts, and products; workspace Clippy runs
alongside them. These use the repository's existing package ownership list.
Expensive targets start only after every unit and lint shard passes.

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

Both names of a rename are included. An empty diff selects all. There are no
receipt-reuse fingerprints or path dependency controllers. The selection rules
live in `tools/ci/pipeline/plan.mjs`.

Every selected target owns setup, build, proof, packaging, and one final
artifact upload on its runner. Targets never depend on an unrelated target.
Compiler caches retain Cargo compiler directories and dependencies. A separate
acquisition cache retains verified tool downloads and pinned installations;
staged products and product proof always start fresh.
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
Ubuntu acquisition uses the official archive mirror rather than the Azure
mirror. If a cached package version is no longer authenticated by the
repository, acquisition refuses; this cache is not a historical apt snapshot.

Every setup emits `target/acquisition/<lane>.json` with verified identities,
operations, durations, cache outcomes, and known download sizes. The supported
local entrance remains `cargo xtask ci pipeline setup <target>` (`setup-unit`
and `setup-ci` cover portable checks and CI validation).

The **Measure tool acquisition** workflow compares a cold project cache with
an exact restored cache on a second, fresh hosted runner. It runs when a tooling
PR becomes ready for review, or manually with selected target IDs or `all`.
Its default selection covers apt, validation tools, QEMU, Chromium/npm, Cargo
tools, AVR, and both ESP compiler families. Measurements include cache
restoration and saving, but exclude common checkout and baseline Rust/Node
provisioning. They require matching source, runner image, specification, and
actual tool identities. A slower warm run is retained as a regression rather
than reported as a speedup. The report separately compares warm preparation
against uncached setup alone, so the cost of populating a cache cannot hide
a regression. Existing runner-image tools are part of the
recorded baseline; “cold” does not mean an empty machine.

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
The x86_64 lane also runs the full graphical user journey against the same
verified product IMAGE: birth, workload changes, execution, input, native Masks,
USB connectivity, loss and lull. `cargo xtask make host verify OUTPUT --journey`
reuses that artifact, checks its digest before and after, and binds the guest's
profile/build identities to its manifest. It does not build a second proof
image. The lane retains the journey records and QMP screenshots with the product.
LoongArch uses Ubuntu 26.04 for QEMU 10 or newer; the verifier refuses older
emulators before boot because their large-page translation can corrupt the
bootloader's module handoff.

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
the site and desktop/mobile browser checks. Publication deploys those same bytes
to GitHub Pages after release acceptance; resuming an older release cannot replace
the current site. No website build runs during publication.

Older Three Bodies and technical recordings retain their original identities,
media, and explicit capture limits. Updating their presentation does not turn
them into execution proof for the new release. The website publication manifest
records current source and retained history separately.

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
