# Stable physical Pico USB selection

All environmental variants previously advertised one shared USB serial. Linux
could therefore move the by-id alias between attached boards. The firmware now
reads the existing flash unique-ID interface before starting peripheral tasks
and encodes all eight bytes as a fixed hexadecimal serial, without allocation
or flash writes. Failed/unprogrammed IDs omit the serial; they do not stop local
sensing or introduce a shared fallback. This is physical USB presentation, not
Host/Boot identity, Body admission or authority.

Two CPU identity tests and fourteen carrier regressions pass. Green, White and
Yellow variants link within their unchanged static/stack bounds. Green's exact
final image is physically verified over USB and Wi-Fi. Its by-id alias persisted
across three firmware starts, including the formatter's two image builds. The
initial BOOTSEL/runtime-serial equality assumption was invalid and is retained
as a verification correction; it is not a claimed physical-identity proof.

Green's same-source baseline comparison adds 592 flash bytes. Its dedicated
serial storage is sixteen bytes; all memory grants remain unchanged. White and
Yellow are compilation results only and were not flashed. Firmware/raw status
and original logs remain in the task's physical-identity evidence directory.

Scoped Clippy passes with three existing diagnostic classes allowed. Full
strict lint still refuses the pre-existing Wi-Fi argument-count, fixed-chunk
and non-Zenoh unit-binding diagnostics; it is not reported as passing.

Credentials were explicitly reprovisioned after restart. Native Pico membership,
selected recording, cold-power continuity and persistent credentials remain
separate unfinished product gates. Display ACKs are electrical observations;
no new human visual acceptance was requested or inferred.
