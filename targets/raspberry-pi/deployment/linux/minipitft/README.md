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
