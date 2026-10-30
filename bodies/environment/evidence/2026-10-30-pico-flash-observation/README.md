# Physical Green flash observation and prepared display

Green is running build `986781bb486f10e2dc2e2ad8ce588a8ebbdd5efe8ec030015422dcda8f016694`
with unchanged checked Host configuration `7609e087...` and physical USB serial
`e6639c758f40702c`. The image combines the published prepared-display path with
read-only JEDEC observation before peripheral/core1 tasks start. It reports the
raw observed ID separately from configured geometry and explicitly reports write
timing unqualified. No persistent application writes are enabled.

The exact firmware UF2 passed the shared layout parser before installation.
Every page was within the ordinary firmware region; no credential or Body
bootstrap page was included. The original qualified recovery UF2 was retained.
Linux denied raw picotool access, and the ROM volume has no CURRENT.UF2, so a
complete flash backup/readback was unavailable. No byte-for-byte preservation
claim is made for data regions. This limitation is retained in the private
installation record rather than inferred away from successful boot.

After reboot, the build/configuration/physical USB identity matched. Local
sampling and HT16K33 ACKs resumed, Wi-Fi rejoined the authorized saved profile
through the existing private client, and HTTP observation after closing USB
showed advancing samples and frames without sensor/display faults. Heap peak
remained 4,524 bytes. Exact initial/later counters are in `proof.json`.
This proves electrical ACKs and returned readings, not human optical acceptance,
full power-cycle continuity, installed Body Plays or Host membership.

Release builds pass for the checked Green, White and Yellow profiles; only Green
was installed. Green text/BSS are 636,396/97,232 bytes. Raw build logs, firmware,
UF2 and image hash are retained locally in `target/pico-flash-observation-proof`.

The observed ID is `0xEF4015`. The manufacturer identification table in Winbond's
[W25Q16JV datasheet](https://cdn-learn.adafruit.com/assets/assets/000/116/709/original/datasheetQspi.pdf)
(page 19, revision H) maps EF/4015 to W25Q16JV-IQ/JQ. Page 62 lists maximum page
program and 4 KiB sector erase times of 3 ms and 400 ms. These chip specifications
are inputs to a future timing review, not an admitted aggregate operation bound:
the pinned HAL's core1/DMA/XIP waits and the application schedule still require
qualification. The PDF and inspected table images are retained locally.
