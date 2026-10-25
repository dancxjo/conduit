# Persistent std worker retirement fixture (#6409)

`targets/std/tests/region_worker_retirement.rs` uses the actual public
`conduit_std_host::StdHost`, `InitializedRegionService`, normal checked region
selection, resource admission, issued Play identities and real OS worker
threads. Its small deterministic Backs are fixture implementations advertised
through `StdHost::from_advertisement`; it does not replace StdHost with a test
shell or claim an installed product workload.

The fixture retains a real offered value and sequence from A, then exercises
two terminal paths: a Back step panic followed by a cancellation panic, and a
cancellation panic without an earlier step panic. Shutdown retains explicit
failure/cancellation evidence, joins both threads, disposes each Back once and
releases service reservations before B is admitted on the same surviving Host.
A sibling Back never steps. B refuses A's retired physical Plan under the new
service admission, obtains a newly issued Plan/Play, refuses A's retained
delivery without a current B offer, and subsequently runs a fresh Back while
its sibling remains untouched. A's disposal counters stay unchanged.

| Retained state | Boundary exercised | Limitation |
|---|---|---|
| A worker/kernel and failed cancellation | Exact retirement result, one cancel attempt and one Drop per Back | A cancellation panic is not successful rollback or proven hard termination of arbitrary code |
| A provider/Plan | Re-preparation against B's actual current offer refuses before Back stepping | Candidate preparation/destruction can occur before admission refusal; no zero-allocation/preparation claim |
| A offered cord and sequence | Delivery replay with no B offer refuses without stepping B/sibling | Raw cord/sequence is not a cross-owner capability; this does not prove stale callback rejection when an owner incorrectly routes a matching B transaction |
| B identity and authority | Fresh public service selection/admission/start on persistent actual Host | Surviving Host is not a surviving semantic Body/replan proof |

Run through the existing std integration-test entrance:

```sh
cargo +stable test -p conduit-std-host --test region_worker_retirement --locked
cargo +stable clippy -p conduit-std-host --test region_worker_retirement --locked -- -D warnings
```

The focused test preparation passed both cases on Linux using a dedicated
target directory, two build jobs, zero debug information and disabled
incremental compilation. Focused strict Clippy also passed with warnings
denied. Each successful case emits a small JSON witness with
the actual old/new Plan and Play identities, retained cord sequence, cleanup
counts, proof class and missing boundaries. Panic-hook messages are expected
negative evidence, not test success being inferred from a timeout.

This is hosted cooperative worker evidence, not physical isolation, protected
native execution, provider/DMA quiescence or hard termination. The public region
worker API has no Host Call completion/callback/Watch operation. No such replay
proof is inferred from cord commands. Those #6409 cases require their actual
effect/callback owners and a separate persistent A→retirement→B fixture.
The wider #6409 exhaustion, composite child-rearm, native broker fault and
semantic-Body/physical-authority transition matrix remains open; this lane does
not change #6311/#6317/#6184 continuation or preemption mechanisms.
