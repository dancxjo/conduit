# Increasing remote fan-out target admission

Development slice of #5231 bounded dispatch/remote pressure work, 17 October 2026.

Payload and terminal remote fan-out previously checked each (endpoint, cord)
against its entire earlier target prefix. A 10,000-target increasing list
performed 49,995,000 tuple comparisons before payload/terminal publication.

A transient maximum-seen CordId now proves an increasing target cannot equal
any earlier tuple. Such targets skip the prefix scan. Targets at or below that
maximum retain the exact tuple comparison against the original prefix. The
helper runs at the original validation point in both payload and terminal
fan-out: caller target order, identity/sequence/pressure/error precedence and
atomic publication are unchanged. It does not sort caller data, deduplicate
by endpoint alone, or accept repeated tuples.

Production state is one transient Option<u16>, not a per-cord allocation or
persistent scheduler index. Increasing admission has linear scalar work and
zero prefix tuple comparisons. Unordered lists retain the existing quadratic
worst case. Tiny-host timing equivalence and wall-clock speedup are not claimed.

The differential test enumerates 87,381 sequences of lengths zero through eight,
using repeated tuples, different endpoints on the same cord, unordered cords,
and CordId 65535. Every result matches the original exact-prefix oracle.
The 10,000-target case verifies zero prefix comparisons, then appends the final
target again and verifies rejection after 10,000 fallback comparisons.
Temporarily restoring unconditional prefix traversal reproduces 49,995,000
comparisons versus the expected zero; the bound is restored afterward.

Both complete local kernel suites pass under packed and linear ready selectors,
with strict all-target kernel Clippy and formatting/diff checks. Existing remote
fan-out admission/closure/refusal suites exercise the two production call sites.
The large case measures the actual shared duplicate helper, not a complete
10,000-target publication or end-to-end Plan/Play throughput. No CI dispatched
or awaited; parent acceptance and stable/physical/browser proof remain open.

## Production 10,000-target admission and closure

`scan_characterization::remote_many` now exercises both optimized production call
sites with 10,000 actual ingress cords/nodes. After all nodes reach idle, one
payload admission wakes every exact sink and stores one byte with 10,000 shared
references. A second payload returns retryable Full. A wrong endpoint at the
last target, or a duplicated first target at the last position, returns
InvalidRemoteCordAccess despite earlier pressured branches. Payload and closure
refusals leave every queue, sequence, producer-open flag, shared value and Sign
count intact.

Normal closure records one exact lifecycle event per branch while preserving
queued values. Repeating the same closure records nothing; a conflicting
abnormal closure refuses. All 20,000 remote lifecycle records match exact
endpoint/cord/direction/sequence and their ordinary event identities. Sign
sequences are continuous with no retention gap. Global cancellation clears all
value ownership and readiness. The measured admission/refusal/closure/cancel
phase performs one successful one-byte store and zero allocations/reallocations.

Packed and linear selectors emit the same normalized observation: 10,000 targets,
20,000 remote Signs, 30,002 ordinary Signs including startup/cancellation, one
stored byte, zero final values, zero measured allocations. Both complete kernel
suites and strict Clippy pass; the expanded focused case also passes under both
selectors. The diagnostic test thread reserves a fixed 128 MiB stack before the
measured phase; this is not a production stack-admission or physical-memory claim.
The waiting Backs do not consume/complete their input terminals; cancellation
performs final retirement. No full Plan/Play throughput or timing claim is added.
