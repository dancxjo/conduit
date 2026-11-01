# Network meaning and station execution

The network Kinds describe credentials, joining, and the resulting attachment.
Offers describe implementations; they do not initialize a radio or grant authority.

`cargo xtask prove environment network-attempt` checks the portable execution
mechanics for one already admitted station operation. Association and address
configuration share one deadline. An AP switch withdraws AP services first;
failure drops the pending operation before attempting restoration under a
separate deadline. Restoration failure remains distinct from the original
attempt failure. There is no implicit retry or successful attachment on failure.

Root supplies the owned provider, authority, deadlines, and lifecycle policy.
The absolute-deadline guard checks before and after each poll; the clock provider
must arrange an expiry wake. It cannot preempt synchronous native work within
a poll. Provider operations must be cancellation-safe.

The deterministic CPU tests cover phase failures, storage pressure, cancellation,
late completion, and restoration. They do not prove physical radio timing,
SDK memory bounds, an installed Body, or connectivity.
