# Retained Source planning restoration

Implemented in the integration worktree only; no running Root, device, or external Face worktree changed.

The original prepared File capture and caller IMAGE guards now travel through cold owner recovery as an opaque token. Recovery uses the same checker and complete selected entries, requiring their actual workset to equal the currently committed Body. The prior CAS expectation is not replayed. Original Host/Boot/IMAGE guards remain required. No invitation, member return, Play, Source adoption, or one-shot operation is replayed.

Authenticated member admission/return refreshes local Fore planning boundaries from that original checked intent and current offers. Missing members leave planning unavailable without falsifying already committed membership. Cached planning evidence is cleared for missing/empty custody, changed workset, or stale Root/member offers. Neither refresh nor restoration creates a Plan, Wakes, starts a provider, or constructs Fore execution authority.

## Qualification

- Default product check passed.
- Product check with `root-allocation-observation` passed.
- Two explicit tests using a real product Source worker, independently digest-pinned in the fixture's checked IMAGE, passed. An ordinary Rust unit test binary cannot act as that worker, and is not substituted.
- Actual worker artifact: release `373eb90baa33c00c998d56f3b274b8804d122e6967dd29e674677f1d499c5220/conduit-linux-x86_64`, read-only; this is not relabeled as the integration build.
- Tests cover actual prior-Boot admitted owner continuity; original IMAGE/caller ownership; copied labels without counters; changed snapshot/current workset; absent target; signed canonical invitation/member commit with target still missing; successful target appearance; fresh versus stale offer generation; no planning or execution effects; Source change invalidating the old cache.
- Restoration-only allocator observation: 50,865 allocations, 731 reallocations, 6,982,528 cumulative requested bytes, 3,584,443 peak live requested bytes, 20,486 retained live bytes. Peak is within the original actual 16MiB caller reservation. This excludes cold IMAGE/catalog initialization, the separate Source worker process, allocator/platform overhead, and the overall Root resident baseline. It is not a target or whole-Root memory-fit claim.
- Production allocation-observation configurations also reject restoration when their existing process requested-byte peak exceeds the original caller envelope.

The actual Fore actor/native Start integration and live deployment are separate gates. The active Root is unchanged; any final Seed must be prepared/pinned against the deployed final Root peer, not the previous Boot.
- Strict `cargo +1.98.1 clippy --locked -p conduit --bin conduit --tests -- -D warnings` passed (2m29s); product/test target and required dependency graph only.
- `git diff --check` passed. Sources frozen and CPU cache released after qualification.

Logs: `retained-local-fore-tests-20261116.log`, `retained-local-fore-observed-check-20261116.log`, `retained-local-fore-clippy-20261116.log` in this outputs directory.
