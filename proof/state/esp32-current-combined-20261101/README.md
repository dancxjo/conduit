# Current complete ESP32 offline diagnostic

Exact source `2ff950a91d017c5d0c38eb317777600e10ad21e4` builds and packs the full `flat-network-appliance` profile using the preserved Xtensa cache, pinned toolchain and original private Blue construction inputs. It is an old-Body fit diagnostic, not the final one-Hamm provisioning image. No device, UART, reset or flash action occurred.

App: **3,364,288 / 3,371,008 bytes**, 6,720 bytes remaining. Main stack region: **37,372 / 36,864 byte minimum**, 508 bytes margin. Build finished in 7m01s. Complete ELF/map/source and input checks passed. These bounds do not demonstrate lower actual Mask peak, setup success, physical execution, or completion of #6341.

The initial exact3799 build correctly refused stale standalone Cargo.lock under `--locked`; its ignored private log remains preserved. The only source repair adds already-declared `conduit-kernel` to body-make and `conduit-body` to environment-face lock dependency lists. Raw logs, generated inputs and firmware remain in the private directory named by the compact manifest; no private bytes are tracked.
