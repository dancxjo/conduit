# Local environment stewardship

These operational rules apply to agents maintaining a workspace. New contributors
can start with [CONTRIBUTING](../../CONTRIBUTING.md); this is not a setup checklist.

Agents must assess the actual environment, ownership, active processes, and storage pressure when deciding when and how to clean. A `.local` hostname suffix is not required and does not by itself establish ownership.

On Dan's machines `forebrain`, `victus`, and `envie` (including their fully qualified hostnames), agents are responsible for proactively maintaining usable disk space. Check headroom before substantial builds and recover space from verified disposable artifacts when needed; do not wait for disk exhaustion or ask again for routine cleanup permission. This responsibility does not authorize deleting uncommitted work, unique evidence, or personal data. These hosts own their workspaces; agents may manage workspace lifecycle according to the host's local policy.

In cloud, CI, hosted runner, and other managed environments, the agent's own environment-specific rules, permissions, and cleanup lifecycle govern. This repository imposes no blanket prohibition on cloud cleanup and grants no additional privilege there. Follow those environment rules for workspace and cache retention, shared resources, and disposal. If the environment or ownership is uncertain, investigate and limit cleanup to verified disposable artifacts within the agent's authority; report any remaining blocker.

On user-owned machines, recover space in this order:

1. Inventory filesystem usage and the largest directories before changing state. Check for active Cargo, compiler, browser-proof, VM, and other processes that may own candidate artifacts.
2. Remove regenerable build outputs such as inactive Rust `target` directories and tool caches. Preserve outputs used by a running process. Do not delete source trees, repositories, credentials, downloads, virtual-machine images, or other user data merely because they are large.
3. Empty desktop trash and remove stale user-owned temporary artifacts. Do not disturb active sockets, sessions, system-owned temporary paths, or recent artifacts whose ownership is unclear.
4. Use package-native cleanup for package caches and bounded journal retention when available. Do not bypass missing privileges or turn a cleanup into an operating-system reconfiguration.
5. Report the before/after free space, what classes of data were removed, what large candidates were deliberately preserved, and any cleanup blocked by permissions.

Disk cleanup is machine maintenance, not permission to change Conduit source or enlarge an issue's implementation scope.

## Worktree lifecycle

Treat each worktree as a temporary, owned workspace, not a permanent build
cache. Use the existing worktree for its assigned task when practical; do not
create duplicate worktrees just to restart a session.

- At task start, inventory registered worktrees with `git worktree list` and
  check free space on the filesystem that actually holds build output. Before
  expensive builds, account for expected output growth, including nested
  target directories, cross-compilation, and concurrent builds. If there is
  insufficient headroom, clean verified disposable output first or report
  the blocker. Do not wait for ENOSPC.
- At task completion, handoff, or abandonment, inspect the worktree's size
  and ownership; remove superseded, regenerable compilation output when
  no active process or proof consumer needs it. Check custom Cargo target
  directories and architecture-specific build profiles, not just
  `target/debug` and `target/release`. Use exact inspected paths.
  Never run blanket `cargo clean`, `git clean -fdx`, or recursive deletion
  over a mixed build/evidence directory. Retain required evidence according
  to the proof-output lifecycle below.
- Before retiring a worktree, refresh relevant remote refs and confirm
  that its owner has finished, no session/build uses it, no active PR or
  release needs the checkout, and tracked, untracked, and ignored contents
  have been inspected. Preserve dirty/untracked work separately and verify
  that the exact HEAD is reachable from a kept local or remote ref or a
  verified local bundle outside the worktree. A squash merge, age, or a
  clean tracked-file status alone does not prove custody.
- For a Codex- or tool-managed worktree, use its supported retirement
  mechanism if available. Otherwise, from a different checkout of the
  same repository, use `git worktree remove <path>` without force.
  Respect locks and refusals; investigate rather than bypass them.
  Do not retire a worktree still required by the current agent session;
  hand the retirement action to its owner instead.
- Inspect stale registrations with `git worktree prune --dry-run --verbose`
  before pruning. Prune only confirmed stale entries; do not prune a
  temporarily unavailable checkout. Pruning metadata does not reclaim
  live worktree files. Never delete a local or remote branch as a side
  effect of routine worktree retirement.
- In the handoff or completion report, state what was reclaimed or
  retired, the remaining free space if cleanup was needed, and any
  worktrees retained for active ownership or unresolved custody.
  Do not generate a separate committed report for routine housekeeping.

## Proof-output lifecycle

Apply [evidence retention](evidence-retention.md) before generating or transferring
captures. Keep run output under ignored `target/`, not dated source directories.
Record useful outcomes in compact summaries; retain raw bytes externally only
when a concrete consumer or unresolved failure requires them. Once external
custody and checksums are verified, remove duplicate downloads, superseded
captures and disposable local bundles when no running process owns them.
Do not turn routine cleanup into another committed evidence package.

An archive removal on current `dev` is intentional. Agents continuing on older
branches must preserve it: transfer source changes without resurrecting old
captures, and link existing durable archives rather than uploading them again.
Do not force-add ignored output or move it elsewhere to bypass retention rules.
Protect unique required evidence and recovery images locally; that protection is
not permission to commit them or retain redundant copies indefinitely.

## Remote branch cleanup

Do not turn local disk cleanup into a new set of origin archives or recovery
maps. Before publishing a preservation ref, compare its source with current
`dev` and active consolidation work. Keep unique uncertain work under a local
ref or a verified local Git bundle outside the worktree instead. A bundle must
contain the exact commit and pass `git bundle verify` before it replaces a
worktree's custody; dirty and untracked files require separate preservation.

Do not recreate an audited discarded ref under another name. Remote branches
are for current owned work, not every checkpoint or capture. Retire redundant
aliases with expected-head checks after verifying source retention and active
PR/release dependencies. Salvage actual missing code/tests before removing their
only retained source; keep live publication and automation-owned active refs.
