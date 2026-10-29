# Mini PiTFT diagnostic on the original B+ Linux Host

This opt-in ARM1176 diagnostic runs the checked ST7789 source from #6523 through
the same protocol kernel and frame encoder as the native image. The existing
Face scene/typography renderer supplies a finite 240×240 diagnostic frame.
Linux owns `/dev/spidev0.0`, GPIO22 backlight and GPIO25 D/C, with both input-only
button contacts reserved. Source cannot select pins, SPI mode or speed. Linux
uses mode 0, 8-bit words and a conservative 4 MHz bus. Do not run another panel
owner concurrently. Kernel SPI/GPIO access is a cooperative trusted boundary;
this diagnostic is not hostile-code isolation.

Cross-compile on an x86 Linux development machine with the same cross 0.2.5,
Docker image and Rust prerequisites as [the B+ Host](../README-BPLUS.md):

```sh
cargo xtask make host release --platform linux-armv6-minipitft \
  --output /path/to/minipitft-diagnostic
```

Transfer the binary to `~/.local/lib/conduit-minipitft/` on the Pi. The normal
Host is independently installed with its own durable identity. Pass that
installed Host ID to the diagnostic; never copy another Host's state. SPI must
be enabled and the user must belong to `spi` and `gpio`. The exact original
Model B Plus Rev 1.2 is checked before device admission. GPIO character-device
line handles reserve all four contacts. An advisory lock excludes another
instance of this diagnostic from the same user session.

The diagnostic compiles/prepares its embedded checked Source on the Pi at
startup; it performs no Rust/toolchain compilation there. Allow startup time
on the single-core ARM1176. The Linux containing loop has a finite 15-minute
wall-time envelope; native image timing and protocol operation limits are unchanged. It emits stage numbers to stderr and prints
`display-transfer-complete` only after the checked plot reports all 225 frame
chunks and backlight enable. It retains ownership until SIGINT/SIGTERM, then
cancels the play and drives outputs low before releasing them. Abrupt power
loss/SIGKILL cannot run userspace cleanup.

A transfer completion proves software execution and successful Linux transfer
returns, not visible panel geometry or colors. Have someone inspect the panel:
`ConduitOS`, `Mini PiTFT display test`, `Buttons pending`, with four corner
markers. Buttons, environmental telemetry, normal acknowledged Mask/Show and
permission to offer this attachment to an environmental Body remain separate
work. The native ConduitOS image and Linux diagnostic use identical authored
ST7789 files; Linux adds no device command interpreter.

For automatic startup, replace `@HOST_ID@` in the adjacent user-service template
with the installed local Host ID, save it as
`~/.config/systemd/user/conduit-minipitft.service`, and enable it through
`systemctl --user enable --now conduit-minipitft.service`. The normal Host
installation must already have enabled user lingering for startup without SSH.

## First Linux B+ observation

On 28 October 2026, `beeplus.local` completed all 225 checked frame chunks and
reported successful SPI/GPIO returns. The attending user observed a backlit but
blank square 1.3-inch panel. An independent short reference color-bar probe was
also blank in mode 0 at 4 MHz and mode 3 at 500 kHz. The Pi reported no
undervoltage. This is **not visible-display acceptance**; electrical seating,
panel response and the Linux attachment path still need investigation. The
software conformance suite passed all eight steps (84 test executions).

The same SD card was subsequently moved, with the original B+ powered off, to
an exact Pi 3 Model B Plus rev 1.3. Machine identity and environmental observer
state were preserved. The square panel was still blank with the installed
Python spidev implementation and Adafruit's unmodified ST7789 protocol driver.
Raw GPIO23/24 observations confirmed each button, both together, and releases;
GPIO level sampling during SPI transmission confirmed changing CS0, MOSI and
SCLK levels. The panel or ribbon is suspected faulty; no visible acceptance is
claimed. Temporary probes were stopped and backlight/D/C driven low.

## Pi 3B+ with a migrated 32-bit card

The ARM1176-compatible executable also runs on the Pi 3B+ under 32-bit Raspberry
Pi OS. It has its own exact target identity,
`std/armv6/raspberry-pi-3-model-b-plus-rev-1.3`; the original B+ identity and native
MMIO/image fence stay unchanged. `armv6` describes this executable's minimum
instruction/ABI profile, not a claim that the Pi 3's processor is ARM1176.
The Linux diagnostic accepts these two exact board model strings.

The existing `cargo xtask make host release --platform linux-armv6 --output DIR`
producer now emits both exact target manifests for the shared compatible binary.
On the migrated card, install `raspios-pi-3-model-b-plus-rev-1.3-32.json` through
`conduit host service install MANIFEST --state-dir EXISTING_STATE_DIR`.
The normal installer preserves the existing Host identity and private control
state. Never run a copied initialized card simultaneously on both machines.
Environmental native-observer return remains its own supported client operation.
