# Pending owner action with live presence

The browser carrier now permits one in-flight owner action while continuing to
process credential-checked presence renewal and leave frames. The owner IPC
call still returns its exact result; Todo acknowledgement still waits for the
durable checkpoint/terminal receipt. A bounded result channel returns that
outcome to the carrier, which remains the sole socket writer.

At most eight action workers exist process-wide, each requesting a 2 MiB stack.
A permit stays with its operation through disconnect and unwind. One follow-up
frame may be retained, with an encoded extent at most the existing 60 KiB owner
document bound; additional/oversized deferred input refuses. Existing typed
field and decoded Plan limits also apply. Polling timeouts do not consume frame
budget or renew a lease. The 2-second lease, frame count, membership and current
Show/action checks remain. Existing IPC deadlines remain 8 seconds for Todo and
2 seconds for other actions. A lost worker/reply is outcome-unknown, with no retry.

Thirteen participant/admission/route/transport checks pass. The final focused
run passes all four new pending-operation/permit checks and the real partial
WebSocket frame timeout case, plus two existing pending-document/output checks.
Strict CLI Clippy passes. The partial-frame test sends half a masked frame,
waits for a polling timeout, then releases its remaining bytes and verifies the
same decoded message. Staging alone cannot produce acceptance; a second worker,
extra deferred frame and oversized deferred input refuse in the contract tests.
These are local checks. Rebuilt installed-owner/browser verification is pending.
