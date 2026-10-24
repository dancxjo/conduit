# Public persistent-Host region lifecycle proof (#6409 slice)

One actual StdHost admits A, runs its sealed pure-transfer fixture, consumes
shutdown and then admits B. The proof checks actual destruction of both A Backs
before reuse, retained immutable retirement evidence, stale A Plan refusal,
fresh provider/resource generations and Play identity, B progress and exactly
four total Back destructions.

The public integration test passes on its first run; strict owning Clippy,
formatting and diff checks pass. Exact Source hashes, commands, raw log hashes
and actual test executable identity are in `receipt.json`. The executable is
sealed outside the build cache.

This uses the existing sealed fixture Plan and cooperative Rust workers. It does
not prove fresh authored Source checking, late callback/Host Call/Watch/staged
output rejection, panic/failed cleanup, hard termination or memory isolation,
semantic Body replanning, or full #6409 acceptance. The HTTP fix is in separate
PR #6425; this proof remains a recoverable preparation branch until integration.
