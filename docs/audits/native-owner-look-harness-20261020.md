# Installed-owner Look producer: implementation, not live acceptance

For #5628, the existing coordinated native Todo Face producer accepts
`--look-selection`. It sends four actual Shift+F7/F6/F7/F6 requests through the
existing four-refresh owner bearer, requires actual current Show acknowledgement,
validates complete typed Face/MaskShow correspondence, unchanged component and
recipe, fresh Show/Plan identities and stable Host/Boot/generation. It rereads
the installed owner's complete Todo Face after every swap, refuses any changed
Face or Body action, and captures four real QMP PNGs with hashes. A read-only
route, fallback, timeout, duplicate record, pressure or palette/scale mismatch
refuses rather than passing. The producer first checks that installation.json
names the exact executable and guest source revision. None of this adds runtime
authority, actions, grant lifetime or transport budget.

The existing bounded serial/readiness observation helpers moved unchanged into
`owner_action_observation.rs`; orchestration is now below 500 lines. The new
Look producer is separate. Existing Todo Face proof receipts also retain the
complete independently read Face, rather than only its identity/count summary.

The correct owning test target is the xtask binary; the small Make package does
not compile its development CLI. A first Make-package command and a subsequent
filename-based test filter selected zero tests and are not behavioral evidence.
After the full xtask test binary compiled successfully, selecting its actual
`commands::conduitos::owner_action_proof::` module ran **seven tests, all passed**,
including the two new installation/source and record pressure/completion checks,
existing stale/action correspondence checks, QMP option and checkpoint isolation
checks. Formatting and diff checks passed. [Retained compile log, actual selected
test results and prerequisite observations](evidence/native-owner-look-harness-20261020/).

## Blocked live step

No live owner Todo Look acceptance is claimed. A fresh installed product through
the supported Linux release entrance is currently blocked: neither
`aarch64-linux-gnu-gcc` nor `aarch64-linux-gnu-g++` is on PATH. The existing Linux
release command explicitly requires both before compiling. The host has CMake,
but noninteractive administrator access reports that interactive authentication
is required. The supported `cargo xtask setup linux-release` installs those
system prerequisites; no credentials or alternate fabricated installation are
used here. This leaves #5628 and #5994 step10 open; #6156 also retains its queue
dependency. Per the user's instruction, work proceeds to the next actionable P0,
#6180, instead of treating this local blocker as completion or a global impasse.
