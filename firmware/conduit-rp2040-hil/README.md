# Conduit RP2040 HIL firmware

This package links the issue-#28 representative static plan into an RP2040
Cortex-M0+ ELF. It uses USB CDC with VID:PID `1209:c028` for the bounded HIL
protocol. It is test firmware, not a resolver, provisioner, or updater.

The shipped artifact is the generic RP2040 profile. Its executable path
initializes clocks and USB CDC. Its fresh capability report therefore names
the exact generic board profile, firmware identity, embedded executor profile,
target, ABI, and fixed SRAM pool, and advertises no Wi-Fi, AP, CYW43, or
Zenoh-Pico service. Pico W support requires a separately identified firmware
artifact that actually links and initializes the radio/network stack and
reports only its finite initialized services.

Build and inspect the exact artifact:

```sh
cargo xtask embedded-gate
```

The current fixture full-plan hash is:

```text
sha256:9a413d9dbe0986ff14e47bda7ced704241db96261429d3c39998cef91fa9694f
```

Flash `target/thumbv6m-none-eabi/release/conduit-rp2040-hil` using an
operator-reviewed RP2040 procedure. Conduit does not place the board into
BOOTSEL, flash it, enroll it, or configure its host.

After the USB CDC device appears:

```sh
cargo xtask rp2040-hil \
  --require-hardware \
  --expected-plan-hash sha256:9a413d9dbe0986ff14e47bda7ced704241db96261429d3c39998cef91fa9694f
```

The command recomputes the expected release-firmware identity from the current
lockfile, core/executor/firmware sources, memory layout, compiler, target, and
profile. It succeeds only after checking that identity, a fresh capability
report identity, nonce/plan/boot/run attribution, contiguous evidence,
semantic values, queue pressure entry/clearance, node completion, and
successful terminal evidence from the physical device.
