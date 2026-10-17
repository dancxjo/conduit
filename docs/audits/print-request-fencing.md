# Finite print request fencing

Development scope for #5558. The portable `PrintAttemptRegistry` retains a
fixed admitted array of 1..32 attempts and delegates every provider report to
the existing `PrintAttempt` state machine. It owns identity/lifecycle custody,
not provider selection, resource reservations, document bytes or print I/O.

Within one explicitly owned registry scope, each job ID and each attempt ID
can be admitted once. A different payload or attempt ID cannot make a retained
job new. Duplicate job, duplicate attempt, exhausted capacity, unknown attempt,
unsupported admission and invalid lifecycle transition have distinct typed
refusals. Refused portable admission consumes no slot. A full registry refuses;
it does not evict a completed, cancelled, rejected or uncertain attempt.

After dispatch, unknown acknowledgement, provider loss and cancellation retain
uncertain delivery. They do not authorize another dispatch or release identity
custody. Before-dispatch cancellation remains definite. Resource observations
and generic pool membership do not change that protocol outcome. In-flight
loss and the eligibility of a genuinely different later request are separate
facts, even when both use the same immutable Plan.

This is an in-memory finite protocol fence. Its owner must keep it while an
operation could be repeated. Creating a new registry is explicit new custody;
it is not evidence that an old uncertain print did not occur. There is no
cross-restart durability, protection against an owner renaming a job, physical
exactly-once guarantee or automatic retry API. A real printer integration must
supply its own admitted durable/provider proof. Existing bounded native IDs
and lifecycle values retain their representations; no allocation-free whole
Host or physical delivery claim follows from a fixed registry slot array.

The corpus uses typed simulated provider reports and the ordinary generic
shared-pool runtime. No PDF is dereferenced and no physical page is printed.
