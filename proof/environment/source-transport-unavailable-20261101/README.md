# Typed Source transport unavailability

Tested source: `06f1a93124db11b3234e55db3944012ba1001ccc`.

The initialized Source retains its session, continuity and authority through typed
transport unavailability. The failed HTTP request is not retried; a later authored
tick may read again. Only timeout, connection refused and transport IO are allowed.
A spawned confined worker must pass checked reap before those errors escape;
uncertain reap remains terminal ProviderProtocol. HTTP reservation release remains
checked before the Source receives the category. All other refusals stay terminal.

Local qualification passed 17 tests, with the actual isolated HTTP worker set via
`CONDUIT_ENVIRONMENT_TEST_WORKER` (hash in receipt). Three product tests exercise
available -> timeout/EOF/refused -> unavailable -> advancing available in the same
concrete continuing Play, same Source subject/session/grants, retaining earlier
canonical observations and avoiding duplicate accepted counters. Identity,
malformed observation, rollback, cancellation and original source authority,
reservation and retirement controls also pass. The freshness loop is unchanged.

Commands used `cargo +stable test`, the environment in receipt, and serial tests:

- `--manifest-path products/conduit/Cargo.toml --features installed-environmental-sources --bin conduit owned_source_`
- Same product command, filter `owned_body_cancels_delayed_source_without_publishing_late_fore`.
- `--manifest-path bodies/environment/host/Cargo.toml --features carrier-source --lib only_transport_refusals_are_observation_unavailability`
- Same host command, filter `source_operator::tests`.

Logs are compressed beside this file. Product process tests used the existing
product lock. Cargo updated the standalone host lock during unit qualification;
its exact tested delta/hash are retained here, and the tracked lock was restored
without publishing dependency changes. No physical fault cause is established.
Combined optimized integration, physical execution and hosted acceptance are
separate gates.
