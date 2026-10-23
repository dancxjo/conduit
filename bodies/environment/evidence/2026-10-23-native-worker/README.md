# Native worker CPU conformance and firmware compilation

The target worker uses one fixed request queue per pulse/timer provider and a
two-slot reply queue. The ordinary kernel alone selects and correlates work.
Provider futures run alongside kernel advancement, with cooperative executor
yields and completion wakeups instead of spinning during a pending clock wait.
A bounded observer sees only committed publication snapshots. It projects truth;
it does not receive logging or execution authority.

The exact worker is exercised with both contiguous and borrowed admission over
explicitly inert CPU providers. Two full 960-sample logical runs drain with all
submissions/completions/publications correlated. Other cases cover pending-wait
cancellation, committed reading publication during the next pending wait,
provider pressure, provider unavailability, revoked pulse possession, refused
worker reuse, and an abandoned invocation that is reclaimed and kept fenced.
Every execution/control case allocates and reallocates nothing during Play.

Missing kernel/native peers refuse before allocation and preserve the remaining
owner. Successful transfer uses one output allocation and preserves the original
kernel pointer. Sizing the exact unpolled control/observer future starts no work.
On the 64-bit CPU fixture the context is 5,808 bytes and that invocation future
is 3,624 bytes; these are not MCU memory or physical execution receipts.

Root now guards allocation of the concrete device execution context, retaining
the same 8 KiB control headroom. Its diagnostic reply distinguishes the context
from kernel/native preparation. This does not claim an actual task slot, admitted
execution stack, release authority, Body start, or hardware execution. Those gates
remain unwired. Firmware compilation, strict test lint, and the public
native-construction proof pass. The target build still reports unused worker
items while task/start wiring is unfinished. The previously verified physical
preparation image remains flashed and unstarted; this worker has not been flashed.
