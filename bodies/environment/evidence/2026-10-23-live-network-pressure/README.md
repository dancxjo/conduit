# Live setup preparation exceeded Root memory

The repaired image booted and completed native admission, preparation, attributed
Start and Release. During the live sensing run a separate internal HTTP fixture
submitted generated credentials for an absent network; `/network` returned 202.
The fixture used no real network credentials and never touched UART.

Subsequent capture ends with `memory allocation of 1920 bytes failed`. Root never
returned the next management-ready event. The bounded carrier correctly reports
incomplete, no verified quiescence, no Lull and no retirement acknowledgement.
No successful observation Face is claimed. This is not an authority-generation
refusal: setup construction ran concurrently with the native heap owner.

The raw capture contains no fixture password. Source/image and the actual HTTP
submission are retained separately from earlier successful retired-Root network
proof. The failure requires workspace custody admission, not a larger observer
window or weaker stack guard. It does not prove active cleanup or recording.
