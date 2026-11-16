# Yellow ordinary carrier-enabled Source firmware

Fresh ordinary Make compiled, linked and converted the same authored sampling, Mask and output entries from clean Source snapshot `cfbff97c4d9a004a47f7e44db02ccc4b69f3a3a3`. Configuration `370055809c7e53536effe0257fdeb7f2c73a524ca34e734022c4acb8d6086cb1` declares the real finite Fore carrier. No neutral RP prototype was included and no device was flashed.

| Physical fact | Actual | Existing grant/bound |
|---|---:|---:|
| Static SRAM | 208,628 bytes | 229,376 bytes |
| Remaining stack reserve | 61,708 bytes | at least 16,384 bytes |
| Heap arena | 110,592 bytes | 110,592 bytes |
| Main Embassy task pool | 90,336 bytes | included in static SRAM |
| Application flash extent | 1,451,776 bytes | 1,495,040 bytes |

All original physical assertions passed. The application UF2 contains only unique RP2040 firmware-region pages, excluding retained history, credentials and bootstrap. Sealed file hashes/lengths, IMAGE/configuration/source pins, original/captured Source hashes and all three entry names were independently checked. The ELF is preserved beside the UF2.

Artifacts: `work/source-member-control-make-20261111/yellow-fore-profile-qualified-20261116`. Detailed public receipt: `yellow-fore-profile-qualification-20261116.json`.

The existing ARM cache was reused; no new cache fork was created. A narrow offline Pico lock update added only current local dependency edges and is committed separately. Shared CPU cache was not touched.

This is compilation and physical construction proof. Actual preparation/Start heap and stack peaks, native Fore traffic, exact history recovery and live admitted Plays remain to be tested. Generic `make host verify` backend dispatch mismatch remains separately documented; no generated JSON was patched.
