# Contributing to Conduit

Welcome. You can help by improving an example, explaining something confusing,
fixing a bug, making an interface easier to use, or adding a carefully scoped
capability. You do not need to understand the whole system first.

## Get something running

Start with the [browser Tour](https://dancxjo.github.io/conduit/tour/) or the
[ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)
to see what we are building.

For local work, install Git and [Rust through rustup](https://rustup.rs/).
Use the toolchain pinned by the checkout; additional target tools are needed
only for the target you work on.

```sh
git clone --branch dev https://github.com/dancxjo/conduit.git
cd conduit
cargo xtask host std
```

The last command builds the product CLI and runs the checked-in Hello Form.
For a browser or native interface, follow [Try Conduit](docs/try-conduit.md).
Rust needs a working native linker. Running Tour also needs Node.js, npm, and
the WASM target (`rustup target add wasm32-unknown-unknown`). Browser conformance
uses the additional pinned tools described in the [browser proof guide](proof/browser/README.md).
`cargo xtask doctor` reports prerequisites across targets and can fail for
optional browser/Pico tools even when the hosted example can run. The
first invocation compiles the repository tooling, so allow time and disk space.

## Find your place

| If you want to work on… | Begin with… |
|---|---|
| Examples and the programming experience | [Forms](forms/README.md), [Tour](products/tour/README.md) |
| Visual interaction and inspection | [Patchbay](products/patchbay/README.md), [Presentation boundary](docs/presenter-hourglass.md) |
| Language, planning, or execution | [Canon](docs/conduit-canon.md), [architecture index](docs/architecture/README.md) |
| A device, Host, or ConduitOS | [Targets](targets/README.md), then that target's README |
| Setup, documentation, or tests | [Documentation index](docs/README.md), [repository map](docs/repository-layout.md), [CI guide](docs/contributing/ci.md) |

The [roadmap](docs/roadmap.md) groups current work and links to the open issues.
Choose a bounded part of an issue or describe the concrete problem you found.
Check open PRs for overlap before substantial work. An issue's acceptance
criteria define that slice; a small documentation fix does not need a new
architecture proposal.

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

- Forms describe meaning; Hosts supply implementations and platform effects.
- Use the existing planner, kernel, and authoritative state for product work.
- Keep resource bounds, failures, and permissions explicit.
- Test the behavior you changed and describe what the evidence establishes.

Keep the PR centered on one outcome. Include necessary failure or boundary
cases when behavior changes. Documentation should explain the result and link
to deeper detail, with commands entering through `conduit` or `cargo xtask`.
Avoid adding new checklists or repeating architecture rules across guides.

## Verify the change

Choose the check that exercises your change. List supported suites with:

```sh
cargo xtask check --help
```

For example, `cargo xtask check form-s3` checks the Form boundary, while
`cargo xtask check input-semantics` checks portable input behavior. The broader
local workspace check is:

```sh
cargo xtask check
```

That is broader than PR admission and may need additional tools. Documentation
changes normally need correct links, runnable examples, and patch hygiene, not
hardware builds. The visual-reference check is:

```sh
cargo xtask evidence docs-verify
```

Describe any check you could not run and the specific reason. A firmware build,
emulator run, and physical device run establish different things. Physical
work also needs the exact target and procedure described in its guide.

## Open the PR

Explain the problem, what changes for the reader or user, and how you checked
it. Link the owning issue when there is one. Include a screenshot or retained
evidence link when it helps review a visible change. GitHub already records
commits and checks; you do not need to copy their identities into the prose.

PR admission is deliberately small. Automation runs combined development
integration and the exhaustive release gate, then publishes accepted products.
Contributors do not need to assemble promotion receipts or manage release
branches. See [CI for contributors](docs/contributing/ci.md) when a real
failure needs investigation.
