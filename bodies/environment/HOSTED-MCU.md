# Linux-owned Body with a native MCU member

This attended profile moves ordinary planning and the Body lifecycle onto Linux.
The ESP32 owns its initialized providers and checked resident software. Linux
admits that actual Host through challenge proof, plans against its current
offers, and transfers a bounded assignment. Root verifies its independently
linked Source, issues native possession, and starts held. Linux records the
ordinary Body start before requesting signed release. The MCU then samples
through the shared execution kernel; bench readings are not substituted.

## Construct and run

With the pinned ESP32 toolchain installed, create a private owner directory once:

```sh
cargo xtask prove environment owner-key /absolute/private/environment-owner
cargo xtask prove environment member-image /absolute/private/environment-owner/owner-public.bin
```

The member ELF is `target/environment-esp32/xtensa-esp32-none-elf/release/conduit-esp32-tm1637-probe`.
Flash that image using the existing ESP32 hardware workflow. The public key is
provisioning data; `owner-secret.bin` stays on Linux. The build checks an explicit
Conduit runtime package allowlist, retains the local indexed task planner, and
refuses linked Source parsing/checking/expansion. Source authoring remains on
the build host. This member image embeds
checked software, not a future Host/Boot, selected Plan or Body authority.

After a fresh MCU Boot, with UART permission, run:

```sh
cargo xtask prove environment host-owned /dev/ttyUSB0 /absolute/private/environment-owner
```

This births a Body with its existing Crèche-generated name, installs Linux as
first host, admits the MCU and verifies one scheduled physical publication.
It then requests signed cancellation/retirement and lulls the Body. An optional
final count, `1` through `960`, runs that many publications; the linked Source
samples once a minute, at most 960 times. SIGINT/SIGTERM requests the same
bounded cleanup. A carrier failure can leave cleanup unresolved; retained
evidence must not be rewritten to claim quiescence.

Each process has a private `runs/<boot-uuid>/` directory. It retains admission
proof, Body biography transitions, signed Root control/publication evidence and
the semantic environmental Face. Re-entry into an already controlled MCU Boot
refuses before birthing another Body. Restart recovery and unattended service
installation are not implemented by this attended profile.

The quantities remain first-class Temperature and Ratio values. Linux applies
the existing Fahrenheit Look and temperature/humidity Masks to that Face,
reporting four segment patterns and annunciators as previews. **These previews
do not write the physical display.** This native run currently admits only the
sampler workset. Continuous display execution, ordinary joining invitations,
external Fore recording and durable reboot continuation remain delivery work.

## Combined display preparation measurement

Build the attended memory checkpoint with the same public provisioning key:

```sh
cargo xtask prove environment member-image /absolute/private/environment-owner/owner-public.bin --display-fit
```

After flashing and a fresh Boot, use the same `host-owned` command. The image
retains the display computations and Fahrenheit/RH Look beside the running
native sampler. Authenticated control replies include `display_fit` telemetry
with explicit `physical_write: false` and `mask_play_installed: false`.
This is a fit checkpoint, not an enabled physical display. After selector and
Source compaction, measured free Root memory exceeds the existing
output-kernel preparation gate. The separate bounded physical check below
exercises actual output; continuous Mask execution remains open. See [the target measurements](make/TARGET_CHECKPOINT.md#compiled-scalar-and-observation-lowering).

The focused CPU construction/output proof, including exact offer equivalence,
stale-scope refusal, packet equivalence and allocation-free execution, enters
through `cargo xtask prove environment display-output`.

## Bounded native display run

Build the combined native diagnostic and, after flashing and a fresh Boot, run:

```sh
cargo xtask prove environment member-image /absolute/private/environment-owner/owner-public.bin --display-check
cargo xtask prove environment host-owned /dev/ttyUSB0 /absolute/private/environment-owner 2 --display-check
```

The Body workset contains the sampler and ordinary output Plot. Both are planned
against actual current offers, held under separate exact reservations, and
released after the same Body start. The output's exclusive local Fore borrows
the actual MCU publication Face; no register commands or invented measurements
arrive over UART. It alternates Fahrenheit and RH through the existing pure
Mask/Look helper, with a three-second wait before the next page. The shared
kernel retains the checked computations and routes across individual Frames;
request identities and the original finite write ceiling survive reuse.

The physical profile admits at most 19,200 Frames. The owner remains attached:
one complete invocation's evidence occupies a 64-record log and a bounded
2,064-byte transport page. Linux verifies the device signature and exact page,
persists it with its signed provenance, syncs both files and directory entries,
then signs an acceptance bound to the Body, Boot, Play, invocation and digest.
Only that acceptance allows buffer reuse. A missing acceptance pressures display
output while the independent sampler continues. Wrong/stale acknowledgements
cannot erase custody or replay writes. Owner control protocol 3 requires the
matching owner executable and firmware; there is no implicit legacy fallback.

The owner records publication commit and observation clocks in the MCU Boot
domain. The Face's existing two-minute age policy determines freshness; a signed
reading older than five seconds no longer terminates a minute-period appliance.
The publication count bounds the attended run. On completion, the owner cancels
output, archives terminal evidence, retires both owners and records Body lull.
Increasing that count does not increase the finite display work ceiling.

This remains an attended physical checkpoint, **without an admitted ordinary
Mask Plot/route**. Final display blanking and a complete semantic withdrawal path
remain open. Confirmed native completions and signed evidence are instrumented
physical execution; LED appearance still requires human observation. The
[measurement ledger](make/TARGET_CHECKPOINT.md#reusable-physical-output-and-evidence-handoff)
records target RAM, flash, stack and the physical run.

`cargo xtask prove environment display-output` covers complete checked-graph
reuse, conditional refusal outputs, evidence encoding, custody, signed
acknowledgements and host age policy. The 960-valid-frame and separate
960-alternating-success/refusal CPU runs use zero execution allocations or
reallocations and have no evidence gaps. They use fixture native completions;
the largest observed complete invocation log is 36 records, not a worst-case
admission claim.

## Pico comparative gate

Use a retained construction birth Sign from the existing environment-image
workflow:

```sh
cargo xtask prove environment pico-maximal /absolute/image-output/birth-sign.json
```

This produces a UF2 in `target/environment-rp2040/thumbv6m-none-eabi/release/`.
It is a memory checkpoint, not a full Host deployment. Its actual PIO GP2
capture provider, initialized monotonic timer, installed Body, resident sampler
software and active Pico W AP remain retained during owned-offer and indexed
Source preparation. The maximal diagnostic uses a fixed 212 KiB heap; the
stack floor remains 36 KiB. The checkpoint also retains an unchanged-196-KiB
comparison so heap growth is separate from compiler savings.
It starts no Play. DHT/display wiring can remain on the ESP32.

The gate is a lower bound: it omits display and the complete admission/control
Root. A failure already here disproves fit for this preparation strategy;
success would still require those owners, actual execution and complete
admission. An allocation fault enters ROM BOOTSEL and retains its bounded fault
receipt; loading the identical image reports that receipt instead of retrying.
See [the measured checkpoint](make/TARGET_CHECKPOINT.md).

## Wireless carrier candidate

`cargo xtask prove environment member-image /absolute/private/environment-owner/owner-public.bin --wireless-display-check` builds the same authenticated Root protocol on one TCP carrier (port 4242), listening on the initialized AP and station interfaces. The existing `/network` page and acknowledged network setup Plot own AP-to-station switching; this profile does not add another credential or radio path. The current mailbox is volatile. Automatic captive-portal opening and durable credential Keep realization are not implemented.

The owner accepts `tcp://192.168.4.1:4242` wherever the serial port was supplied. A retained, signed, lulled biography can be continued using a final `--resume=/absolute/path/body-lulled.json` after `--display-check`. The previous owner must have exited with verified retirement; the new owner uses ordinary continuity under a fresh local incarnation and keeps the Body identity. This is preparation for wireless operation, not evidence of network HIL or unattended recovery.

Do not deploy this candidate as an unattended long-distance appliance yet. Carrier interruption currently terminates the attended exchange. Persistent reconnection must reconcile incomplete signed requests rather than blindly replay effects, retain pending evidence under finite pressure, and verify the expected member on both station and fallback AP paths. Saved network configuration requires a finite Body-lifetime Keep backed by actual durable private storage; the existing volatile mailbox cannot advertise that retention horizon.
