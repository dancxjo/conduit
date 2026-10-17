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
