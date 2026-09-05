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
The report is usable on every machine, including machines where destructive
reclamation is prohibited. Shared compilation-cache selection, active-job
leases, evidence separation and bounded reclamation remain tracked in #2516.
Do not point every product build at a shared target directory until its staging
paths and concurrent executable consumers have been checked.
