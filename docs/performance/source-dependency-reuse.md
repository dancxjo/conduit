# Source catalogue dependency reuse

`SourcePreparationCache` reuses checking and expansion across unrelated Kind
catalogue edits. Exact Source bytes (not just their digest) and the selected
entry remain mandatory. Kind signature, Fore, variadic Fore, projection and
canonical Kind queries retain exact names and values, including absent names.
A relevant value change invalidates that result without a generation counter.

This slice scopes the explicit catalogue lookup seam. Other declaration, Type,
law, import, literal, private initialization and physical contexts still require
complete equality. Expansion also requires the complete Back catalogue. This
is conservative for those contexts; it does not claim minimal invalidation of
all authored subplots or every Type dependency. Catalogue enumeration falls
back to whole-context equality.

One cold retained preparation owns a recorder limited to 4096 observations and
128 KiB of exact UTF-8 key bytes. Exhaustion, unfinished publication or a
surviving internal catalogue observer requires whole-context equality. Successful
results retain only the immutable deduplicated read set; the recorder retires
before publication. Errors are not cached. Zero retention and oversized Source
keep the ordinary cold path. Existing FIFO entry bounds and immutable borrowed
owner lifetimes remain in force. Input catalogues are caller-admitted snapshots;
these entry bounds are not a byte-accurate total memory reservation.

No Plan, Host, Boot, authority grant, mutable kernel or active Play is cached.
Current planning and execution still establish their own exact facts.

## Local cost observation

The [retained observation](../../proof/performance/source/dependency-cache-cost-20261020.json)
compares the original whole-context policy with the new policy using the same
current checker and expander. Three alternating pairs prepare 32 Gears through
64 independently assembled catalogues differing only in an unused Kind. Each
complete checked and expanded value equals an independent cold oracle.

The original policy performs 64 checks and expansions; the dependency policy
performs one of each and 63 hits. Allocation requests are 732,609 versus 11,894.
Peak additional live allocation is 4,793,513 versus 1,752,772 bytes. Both owners
return to the exact pre-cache live-byte baseline on drop.

The separate single-cold observation exposes the tradeoff: the recorder and
catalogue clones add 194 allocation requests and 370,926 requested bytes; peak
additional live allocation rises from 1,569,446 to 1,752,772 bytes. Reuse benefits
require repeated compatible work. Source/catalogue/oracle construction is
outside this measurement, and unchanged between policies. Concurrent compilation
and an uncalibrated machine prevent any product latency or speedup claim.
