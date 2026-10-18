# Abnormal terminal semantic value envelope

Development fix for #5231 terminal/pressure/ownership preservation, 17 October 2026.

An abnormal terminal that fitted a cord's aggregate queue byte capacity could
exceed its smaller `maximum_value_bytes` envelope and still be published. Local
ordinary payloads and remote ingress already enforce this independent semantic
limit. Terminal values now obey it in the existing precommit publication pass.

For each matching abnormal-terminal target, a value exceeding the queue budget
retains the existing `QueueCapacityExceeded` refusal. A value fitting that budget
but exceeding the semantic envelope returns `SemanticValueBoundExceeded` before
Back commit, completion, allocation, queue writes or terminal closure/Signs.
There is no additional route traversal, scheduler storage or runtime allocation.
Zero-byte normal closure remains admissible under positive semantic limits.

The regression uses two remote terminal branches with eight-byte queue budgets
and semantic limits [2,8] then [8,2]. In either position the narrower branch must
refuse the four-byte terminal before any branch observes it. The Back remains
uncommitted, source ready/incomplete, queues/storage empty and only the pre-Step
fuel Sign is recorded. Retrying at exactly the two-byte limit commits once and
publishes one shared two-reference value to both branches with exact Sign count.
Global cancellation clears the value. A normal closure with one-byte semantic
limits also succeeds. Disabling the new check reproduces successful Progress
where semantic refusal is expected; the check is restored for final validation.

Both complete local kernel suites pass under packed and linear ready selectors,
with strict all-target kernel Clippy, formatting and diff checks. This is native
kernel evidence; broader terminal rollback, physical/browser execution, stable
acceptance and parent completion remain outside this claim. No CI dispatched or
awaited.
