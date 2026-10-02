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
Target caches retain only Cargo compiler directories and dependencies; staged
products, proof outputs, and receipts always start fresh.
Once the target matrix starts, a failure does not cancel siblings. Code tests
have no retries. Acquisition may have one bounded infrastructure retry.
Browser acceptance keeps pinned Chromium, one worker, and zero retries.

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

This software release path does not regenerate documentary journeys or replace
the existing `gh-pages` site. That separately retained evidence must not be
relabelled as proof for a new software release. Tested browser products are
included in the release bundle alongside the other targets.

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
