# Local build storage

Run `cargo xtask ci storage-report --locked` from a Conduit worktree to inventory
the `target` directory of every registered Git worktree. The command emits JSON
with sorted paths, logical file bytes, file counts, and skipped symlink counts.
It performs no writes or deletion. Inaccessible paths fail explicitly rather
than silently understating usage. Measurement is a live filesystem observation,
not an atomic snapshot; retry an inventory if an active build changes its files.

Logical bytes are not reclaimed disk space: sparse files, compression and hard
links can make these differ. Symlinks are not followed. External target paths,
standalone nested target directories and Cargo/tool caches are outside this
report's scope. The report does not infer that a directory is disposable merely
because its name is `target`; such directories can also contain retained proof
evidence and active outputs.

Use the working agreement's machine and ownership checks before any cleanup.
The report is usable on every machine. It was introduced through #2516;
read-only inventory and guarded reclamation are separate operations.
Do not point every product build at a shared target directory until its staging
paths and concurrent executable consumers have been checked.

Use `cargo xtask ci storage-reclaim --locked` for a dry-run selection of
regenerable Cargo compilation directories. It preserves the current worktree,
dirty worktrees, heads not reachable from a current remote ref, targets
referenced by live processes, symlinks, root executables, staged products, and
proof evidence. Selection is capped at 10 GiB of logical bytes by default; set a
smaller explicit cap with `--maximum-bytes N`.

On the user-owned `forebrain`, `victus`, and `envie` hosts only, add `--apply`
to remove exactly the selected compilation directories. Apply mode is refused
elsewhere and on platforms where live process references cannot be inspected.
The JSON result distinguishes selected from reclaimed logical bytes and records
every preserved worktree and reason. Refresh remote refs before applying so the
reachability guard uses current remote truth.
