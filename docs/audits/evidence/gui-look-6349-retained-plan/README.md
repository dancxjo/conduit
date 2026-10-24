# Owner action / presence timing experiment

The complete-Look source `3c37cabe5` intermittently loses the 2-second presence
lease after an action. A diagnostic encounter succeeds, but a renewal remains
queued behind the owner action request for 1,315 ms. The maximum browser
event-loop gap recorded before that action is 278 ms. This does not measure
all in-flight browser work. A diagnostic success does not erase the retained
failure.
Timing files contain only frame kind, direction and time.

Experiment source `0059e318f` retained the exact owner-issued Mask Plan across
Face-only refreshes. Its focused CLI contract test passes current reuse and
changed source/root/Host/Line/seal refusal cases. The rebuilt owner and browser
complete eight Look selections and a fresh acknowledged Todo add. However, the
action request still takes 1,500 ms versus 1,533 ms in the baseline; a renewal
still queues for 1,321 ms. The experiment does not establish a lease fix.

The delivery branch removes that runtime optimization. Its source and test
remain recoverable at the experiment commit in this branch's history; the
unmodified complete-Look transport remains. Source-bound successful and failed
encounters remain distinct. No timing threshold or bounded-real-time guarantee
is inferred from these samples.

The current carrier processes `submit_browser_face_interaction` synchronously.
That call intentionally waits for the exact checkpoint output and terminal Sign,
not mere queue admission. During that wait the carrier cannot read or acknowledge
renewals already sent by the browser. The next implementation must admit one
bounded in-flight owner operation while continuing current-credential presence
renewal, preserve the exact commit acknowledgement, stale Show refusal, two-second
lease, finite frame/storage/work budgets and outcome-unknown behavior. This is
an implementation requirement, not a completed concurrency proof.
