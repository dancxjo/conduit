# Type-derived output value backing

Trusted Make derives a conservative 651-byte canonical ceiling from every
checked payload Type at the expanded output graph's ports, including both
variant branches. Unknown or abnormal payload Types refuse derivation. The
independent linked profile supplies that ceiling; incoming assignments do not.
The native constructor and its admission estimate both use it.

Twelve uniform slots remain. Each valid value fits any slot, and the live-byte
quota equals the sum of all twelve ceilings; it therefore cannot refuse a set
of fitting live values before the unchanged slot count does. This proves no
lifetime reuse or reduction in mandatory sign retention. The current payload
reservation drops by 4,476 bytes; allocator rounding and full MCU retained/peak
cost remain separate, unmeasured obligations for this change.

Thirty focused tests pass, including actual assigned ordinary-kernel output,
brightness refusal, pending/cancel paths, too-small-storage refusal before
software transfer and Root reservation checks. Scoped library/display Clippy
is strict. Root-test Clippy permits dead_code for three existing included
firmware items. The display-ready ESP32 release image links; it was not flashed.
The preceding physical capture used 1-KiB slots and remains labelled that way
in the ledger. Working appliance firmware is unchanged.
