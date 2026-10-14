# Contributing to Conduit

Welcome. You can help by improving an example, explaining something confusing,
fixing a bug, making an interface easier to use, or adding a carefully scoped
capability. You do not need to understand the whole system first.

## Get something running

Start with the [body Workspace](https://dancxjo.github.io/conduit/workspace/) or the
[ConduitOS visual journey](https://dancxjo.github.io/conduit/journeys/current/conduitos/x86_64/)
to see what we are building.

For local work, install Git and [Rust through rustup](https://rustup.rs/).
Use the toolchain pinned by the checkout; additional target tools are needed
only for the target you work on.

```sh
git clone --branch dev https://github.com/dancxjo/conduit.git
cd conduit
cargo xtask make host std
```

The last command builds the product CLI and runs the checked-in Hello plot.
For a browser or native interface, follow [Try Conduit](docs/try-conduit.md).
Rust needs a working native linker. Browser proof also needs Node.js, npm, and
the WASM target (`rustup target add wasm32-unknown-unknown`). Browser conformance
uses the additional pinned tools described in the [browser proof guide](proof/browser/README.md).
`cargo xtask doctor` reports prerequisites across targets and can fail for
optional browser/Pico tools even when the hosted example can run. For the full
Linux release set, including the Raspberry Pi OS AArch64 package, run
`cargo xtask setup linux-release` once, then verify it with
`cargo xtask doctor linux-release`. The first invocation compiles the repository
tooling, so allow time and disk space.

## Find your place

| If you want to work on… | Begin with… |
|---|---|
| Examples and the programming experience | [reviewed plots](plots/README.md), [Try Conduit](docs/try-conduit.md) |
| Human encounter and inspection | [universal Face grammar](docs/architecture/face-grammar-conformance.md), [Patchbay Plot graph](plots/patchbay/graph/README.md) |
| Language, planning, or execution | [Canon](docs/conduit-canon.md), [architecture index](docs/architecture/README.md) |
| A device, host, or ConduitOS | [Targets](targets/README.md), then that target's README |
| Setup, documentation, or tests | [Documentation index](docs/README.md), [repository map](docs/repository-layout.md), [CI guide](docs/contributing/ci.md) |

The [roadmap](docs/roadmap.md) groups current work and links to the open issues.
Choose a bounded part of an issue or describe the concrete problem you found.
Check open PRs for overlap before substantial work. An issue's acceptance
criteria define that slice; a small documentation fix does not need a new
architecture proposal.

## Keep ownership and handoffs small

Choose one independently verifiable child issue and check current `dev`, open
PR ownership, and origin branches before branching. Salvage useful existing
work with its discussion and proof intact. Keep at most two overlapping
implementation PRs in review across the flow, with at most one merge-ready PR
changing shared core contracts. Give each a named owner and disjoint scope.
Other tickets stay unbranched until a slot opens or the owner explicitly hands
off; when parallel preparation is requested, use isolated worktrees and hold
prepared changes locally rather than expanding the PR queue.

Move a reviewed result through its required green merge/integration or an
explicit handoff before admitting the next result. While checks run, continue
independent investigation, proof, or requested isolated preparation. If combined
`dev` is red, the shepherd repairs or reverts the smallest cause before feature
admission; preserve held branches and evidence. A handoff names the issue,
branch/head, owned files, completed proof, remaining gate, and next owner.

| Follow-up | Share one PR when… | Separate when… |
|---|---|---|
| Code | A fix and its refusal tests establish the same behavior. | A second change alters an independently owned core contract. |
| Docs | The changed command needs matching instructions and examples. | A broader tutorial has its own runnable acceptance and proof. |
| Hardware proof | A driver fix needs the same exact-target receipt to prove it. | A new board or physical run has independent equipment, ownership, or acceptance; keep that issue open until its real proof arrives. |

Merged PRs retain their discussion and receipts; a successor references them
rather than rewriting another author's branch. Candidate admission establishes
PR eligibility. Integration and publication keep their separate source-bound
proof obligations in the [CI guide](docs/contributing/ci.md).

## Make a focused change

Create a branch from current `dev` and open the PR against `dev`. If you do not
have repository write access, push the branch to your fork and open the same
kind of PR.

```sh
git switch dev
git pull --ff-only origin dev
git switch -c your-change
```

Use a clean checkout, or preserve unrelated local work before switching.
Read [AGENTS.md](AGENTS.md) before changing code. It records the shared
architecture and collaboration rules; the important starting points are:

- plots describe meaning; hosts supply implementations and platform effects.
- Use the existing planner, kernel, and authoritative state for product work.
- Keep resource bounds, failures, and permissions explicit.
- Test the behavior you changed and describe what the evidence establishes.

Keep the PR centered on one outcome. Include necessary failure or boundary
cases when behavior changes. Documentation should explain the result and link
to deeper detail, with commands entering through `conduit` or `cargo xtask`.
Avoid adding new checklists or repeating architecture rules across guides.

## Verify the change

Start with the ordinary local integration loop:

```sh
cargo xtask integrate
```

It runs representative language, planner/kernel, std Host, Body lifecycle,
multi-placement, recovery, Patchbay, and browser/WASM paths without performing
release, publication, emulator, or physical-hardware work. A missing local WASM
target is reported as a prerequisite instead of being replaced by a fixture.

Choose the check that exercises your change. List supported suites with:

```sh
cargo xtask check --help
```

For example, `cargo xtask check plot-s3` checks the plot boundary, while
`cargo xtask check input-semantics` checks portable input behavior. The broader
local workspace check is:

```sh
cargo xtask check
```

That is broader than PR admission and may need additional tools. Documentation
changes normally need correct links, runnable examples, and patch hygiene, not
hardware builds. The visual-reference check is:

```sh
cargo xtask prove docs-verify
```

Describe any check you could not run and the specific reason. A firmware build,
emulator run, and physical device run establish different things. Physical
work also needs the exact target and procedure described in its guide.

## Open the PR

Explain the problem, what changes for the reader or user, and how you checked
it. Link the owning issue when there is one. Include a screenshot or retained
evidence link when it helps review a visible change. GitHub already records
commits and checks; you do not need to copy their identities into the prose.

Draft and ready PRs receive the same formatting, lint, affected package tests
and relevant smoke checks. Native isolation and device ownership changes retain
stronger targeted platform proof. Mark a healthy draft ready when it can be
reviewed and merged. Automation runs exhaustive proof once for combined
development, then publishes those tested
products after verifying their identities; release does not rebuild them.
Contributors do not need to assemble promotion receipts or manage release
branches. See [CI for contributors](docs/contributing/ci.md) when a real
failure needs investigation.
