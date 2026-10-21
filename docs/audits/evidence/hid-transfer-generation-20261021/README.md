# Held native HID transfer-identity preparation

Issue #6182. Exact tested source: `5b874c7807a69dd56fb225140de1609a62c52540`.
This branch retains the earlier MSI-X/RTC preparation and the repaired #6204
baseline; it is not a new admitted PR or complete interrupt/lifecycle delivery.

Keyboard completion acceptance now requires an EventData cookie. A never-reset
checked serial range is reserved before publishing a whole runway window; eight
Root-owned interests bind report ordinal and attachment epoch. Duplicate, foreign,
stale and pointer-only events cannot retire the current interest or publish input.
The fixed 4096-byte DMA remains unchanged: 31 two-TRB TDs use the existing ring,
with eight report buffers. ED completion lengths are transferred-byte counts;
other pointer consumers explicitly reject ED before interpreting a cookie as an
address or treating its length as a residual. Busy interests refuse runway reset.

Focused actual checks: 78 x86 tests, 33 shared xHCI tests and 108 USB tests passed,
zero failed or ignored. The x86 log retains the initial run plus its exact-clean-
head cached verification; 78 unique x86 cases are counted once. Compressed raw
logs and uncompressed hashes are retained beside this record.

This deliberately does not cancel a transfer, acknowledge DMA retirement, stop
or unregister MSI-X, or make a failed session recoverable. Pending interests stay
owned after refusal until the missing acknowledged-stop lifecycle is delivered.
Actual MSI-X/idle qualification, per-report IRQ correlation, native burst/wrap/
revocation proof and matched total CPU gain or honest no-gain remain unfulfilled.
No physical, full native, CI/Integration, publication or #6182 closure claim.
