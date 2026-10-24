# Fixed Source-derived Frame storage

Ordinary composition at build time derives the exact canonical Frame template
and each scalar location from the checked Source type. Changing one scalar
must change exactly one distinct byte, with exact U8/Boolean Types and six
registers. Runtime keeps only canonical bytes and validates each result; no
packet commands or handwritten protocol-envelope offsets move into native code.
The authored packet expression still owns command generation and brightness
refusal. Native local Fore storage now uses the exact Frame extent.

512 independent ordinary-composer comparisons cover every register/brightness
byte and both Boolean states. Runtime tests, twenty Mask/Root-control cases,
scoped lint, thumbv6m no_std compilation and the ESP32 release link pass.
The full timed Mask's 64-bit owner is now 344 inline bytes with no preparation
heap allocation, versus 952 inline plus 3,112 requested heap bytes beforehand.
Recurring rendering and execution remain allocation-free. These are requested
payload measurements, not allocator overhead, MCU size or combined admission.

The isolated dev delivery is PR #6405. Its own six packet/material tests, strict
scoped lint and thumbv6m check pass on its delivery tree. The construction
branch additionally contains the smaller local caller and Mask measurements;
they are not part of that narrower PR. No new firmware was flashed. Actual
Mask/clock/caller/output admission, network transition and recording remain
unfinished.
