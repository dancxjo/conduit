BOT recovery acceptance audit
=============================

This audit is incomplete. It distinguishes an acknowledged transport mechanism
from permission to resume a reviewed storage transaction.

Implemented locally
-------------------

* f6e540267: exact original endpoint Stop acknowledgement; only its sole retained
  TD can be drained, foreign/duplicate events and wrong acknowledgements refuse.
* 2425afc21: exact slot-success Reset Endpoint and stopped-ring dequeue commands.
  The xHCI mechanism suite passed 22 tests. These command proofs use simulated
  controller registers/events, not emulator or physical execution.
* 657dbb6c2: a known single STALL may retire its exact queue after Reset Endpoint
  acknowledgement, keeping EP0 and the attachment. Peer activity, unknown faults
  and control uncertainty retain full-slot sealed retirement. New effects remain
  blocked until the original bulk Kernel obligations retire. All 99 existing
  native USB regressions passed; focused recovery coverage is still pending.

Existing Source policy
----------------------

[mass-storage-reset-policy.conduit](../mass-storage-reset-policy.conduit) authors
reset(interface 0), clear IN 0x81, clear OUT 0x02. Each requires an exact successful
zero-data acknowledgement; a failed clear IN forbids clear OUT. Its retained
standalone emulator proof does not establish interrupted-read reentry.

Unproven composition
--------------------

1. Actual interrupted selected read -> exact queue acknowledgement -> original
   typed STALL -> exact Kernel token/obligation retirement.
2. Original failed storage result must settle truthfully. Native endpoint idle
   alone cannot mint a storage completion or silently revive its authority.
3. The checked reset/clear-halt sequence must execute on the same retained
   attachment, with no old command replay.
4. A separately authorized new read must use a fresh CBW tag/request and retain
   transport cursor, cycle and completion ordinal history. Its checked block
   result must correspond to that new command/data/status, not the old token.
5. Failed acknowledgements, detach, unknown control state, stale TDs and old
   Kernel tokens must prevent reentry; allocation and work remain bounded.

Additional authority gates found in the live implementation
----------------------------------------------------------

`storage_base/transaction_owner.rs::settle` seals the owner after any result
other than completed/incomplete. `PreparedStorageCalls::allows_usb` requires an
active authorized submission and forbids controls after that submission settles.
`dispatch_storage` independently validates original USB subordinate settlement.
The retained stopped queue currently blocks that idle validation until explicit
reentry. Therefore returning Source to its ready phase alone cannot establish
recovery: a fresh storage authorization and truthful quiescence receipt must be
composed explicitly. Clearing the terminal flag or reviving revoked possession
is not evidence of authorization. No replacement authority contract is admitted
by this audit.

The connected changes remain unpublished pending the focused mechanism and
composition proofs. No issue closure or physical compatibility is claimed.
