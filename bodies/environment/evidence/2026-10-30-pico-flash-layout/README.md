# Shared Pico flash layout qualification

The existing 2 MiB Pico/Pico W target now derives linker, embedded Spore,
browser deployment and native deployment bounds from one checked frozen data
module. The runtime crate is `no_std`, allocation-free and has no runtime
dependencies. Serde is used only in build preparation and CPU tests.

The final sector remains the existing Body bootstrap at `0x101ff000`. The two
sectors immediately preceding it (`0x101fd000..0x101ff000`) are reserved for the
shared credential journal. Ordinary firmware cannot link into these sectors;
UF2 admission rejects payload pages there. Sparse browser erasure preserves
sectors omitted from the admitted image. No history region is invented here.

## Qualification

- Layout and host-make CPU suites: 88 tests passed; strict all-target Clippy passed.
- Layout `thumbv6m-none-eabi` no_std check passed.
- Browser custody/workspace/workflow Node suites: 14 tests passed.
- RP2040 deployment Chromium suite: eight tests passed, one worker, no retries.
  This uses the actual optimized browser runtime and target asset, with mocked
  USB equipment. It is adapter proof, not a physical deployment claim.
- Generated linker boundary: exactly 2,084,608 bytes of ordinary FLASH section
  accepted; one additional byte refused with FLASH overflow.
- Green environmental firmware, default installed Pico signal firmware and DHT11
  probe release builds passed. Green text changed from 634,292 to 634,296 bytes;
  static BSS stayed 97,152 bytes. No candidate runtime heap claim is made.
- The installed signal build predates a final identity-only addition of the same
  layout digest to the Pete build identifier; Pete-mode execution was not proved.
- Minimal lockfile changes add the local package and its explicit dependency
  edges, without upgrading existing packages or checksums.

Raw logs, linker boundary artifacts, qualified firmware ELF files and hashes are
retained locally under `target/pico-flash-layout-proof`. The successful browser
runtime is retained under `target/browser-host-release/runtime.wasm`; regenerable
compiler caches were removed after saving qualified artifacts.

## Remaining physical and architectural work

No device was flashed for this qualification. Green continues running its prior
qualified environmental carrier and still reports `body_member: false`.
These checks establish geometry, not FLASH possession, an admitted write window,
installed credentials, retained Body keys, durable observation history or MCU
membership. Exclusive physical ownership and safe flash scheduling must precede
journal writes and power-loss/restart testing. Local sensing and display must
continue throughout that later qualification.
