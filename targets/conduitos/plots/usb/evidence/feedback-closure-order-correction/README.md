# Feedback Zip closure-order correction

Candidate38352158907 exposed an existing USB duplex conformance regression:
a right-first request close followed by its delayed completion emitted the
correct pair, but a subsequent state-side close returned Progress instead
of the established Complete outcome.

Persistent event drain is now entered only when the left/state side closes
before the right/event side. An already observed right close preserves the
original completion law. No assertion or terminal contract was weakened.

Retained results: USB duplex3/3, generic Zip10/10, and whole BOT failed-clear
closure1/1 (one and two original requests). The generic tests retain delayed
payload/real-close drain, pressure, abnormal failure and zero-allocation
checks. These are deterministic Source/Kernel witnesses, not fresh physical
or guest execution. Historical guest receipts retain their original kernels.

Full workspace Clippy with warnings denied and preflight185/185 also pass.
