Recovery archive for local cleanup, 2026-10-24.

Original branch names and worktree heads are in inventory.json. Every original head was verified reachable from an origin branch before local deletion. Uncommitted working files are on archive/cleanup-20261024/wip/* branches; the partial staged face-shell index is on archive/local-cleanup-20261024/staged-face-shell-0dd4d2a9. These are unvalidated recovery snapshots.

Recover using: git fetch origin; git worktree add -b recovered-name /desired/path COMMIT_SHA

Ignored build outputs and proof artifacts were moved locally, without deleting their contents. cleanup-result.json records their paths and any retained worktrees. Remote branches were preserved.
