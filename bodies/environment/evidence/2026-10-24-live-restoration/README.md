# Restored live Face and Mask bench

Built through `cargo xtask prove environment ap-esp32` and flashed the attached
ESP32 `b0:a7:32:28:61:78`. The image receipt pins the source and ELF digest;
the ELF remains in ignored `target/environment-live-restoration/live.elf`.

The 42-second read-only serial capture contains thirteen acknowledged display
writes and advancing sample sequences 2, 3 and 4. All thirteen readings were
current and available: 68.36 degrees Fahrenheit and 60 percent relative humidity.
Alternating register frames were `[125,255,102,113,0,0]` (68:4F, the Look's
colon fractional marker) and `[0,125,63,118,0,0]` (60H). The physical driver
acknowledged every captured write. Operator visual confirmation is separate.

The host connected to the AP with Ethernet retained. HTTP `/reading` returned a
current, available observation at sequence 3 with the same quantities. Both
HTTP and segment representations derive from the same semantic Face.

This restores the attended integration bench after the native readiness
experiment left the old display value unchanged. It samples every fifteen
seconds and selects a new Mask page every three seconds. It has no installed
Body/Plan/Play, native output admission, recorder persistence or network
transition. This short run does not prove overnight endurance. The native
construction goal remains incomplete; do not deploy a readiness-only image as
if it were the continuing appliance.
