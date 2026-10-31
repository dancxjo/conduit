# Persistent std worker retirement fixture (#6409)

[`region_worker_retirement.rs`](../../targets/std/tests/region_worker_retirement.rs)
exercises public `StdHost` and `InitializedRegionService` with checked region
selection, resource admission, issued Plan/Play identities and real OS worker
threads. Its deterministic Backs are fixture implementations advertised through
`StdHost::from_advertisement`.

A retains an offered value and sequence. Two cases exercise a Back step panic
followed by cancellation panic, and cancellation panic alone. Shutdown records
failure and cancellation outcomes, joins both workers, disposes each Back once
and releases reservations. The same Host then admits B with fresh Plan/Play
identities. Re-preparing A's physical Plan refuses; replaying A's delivery when
B has no current offer refuses without stepping either B Back. B's fresh source
then steps while its sibling remains untouched. A's disposal counts stay fixed.

| Boundary | Assertion | Limit |
|---|---|---|
| Failed cancellation and poisoned worker | One cancel attempt and one Drop per admitted Back; explicit retirement outcome | Cooperative hosted proof, not rollback or hard termination of arbitrary code |
| Retired physical Plan | B's service refuses A's old admission | Candidate preparation and destruction may occur before refusal |
| Retained cord and sequence | Delivery without B's current offer refuses without stepping B | Raw cord/sequence values are not cross-owner capabilities; matching live B transactions are not exercised |
| Persistent Host | B receives fresh public admission and Plan/Play identities | Does not establish surviving semantic Body/replan behavior |

The tests belong to the ordinary std integration-test inventory, included in
`cargo xtask check`. For focused contributor diagnosis, the internal owning
package commands are:

```sh
cargo +stable test -p conduit-std-host --test region_worker_retirement --locked
cargo +stable clippy -p conduit-std-host --test region_worker_retirement --locked -- -D warnings
```

Each case emits a compact JSON witness. Expected panic hooks are negative-case
inputs; successful assertions and thread shutdown establish the test result.
The public region-worker API has no Host Call completion, callback or Watch
operation, so these tests do not prove replay fencing for those owners. Physical
isolation, native protection, provider/DMA quiescence and the broader
[#6409](https://github.com/dancxjo/conduit/issues/6409) matrix remain separate.
