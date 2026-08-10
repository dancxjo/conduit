# Netherwick describe-only proof

This crate projects one real Netherwick configuration without acquiring any ability to move it.
It consumes `pete-brainstem::conduit_robotics::pico_w_report()` directly from pinned Netherwick
revision `f43ff13846b47b05e133d0321bdbaafffd1bcdbe`. The represented hardware declarations are that
revision's `crates/pete-brainstem/body.toml` and `board.toml`: an RP2040/Pico W brainstem, an
iRobot Create Open Interface differential body, Create bump sensors, and the MPU-6050 IMU.

The Conduit profile exposes only semantic bump and IMU observation offers. It separately describes
the differential-drive actuator, the brainstem-owned inhibit/safety machinery, and the absence of
both an actionable offer and an authority grant. A valid `robotics/drive-differential` Form fails
ordinary placement before a Plan or Play exists.

The Observatory snapshot carries exact fixture Host/Boot identities, configured USB CDC and Create
UART Bases, the motherbrain-to-brainstem Line, finite resources, and bounded Signs. Because this is
configuration projection rather than live hardware observation, Host, Base, Line, sensors, and
physical safety state remain explicitly `Unknown`/unavailable. The fixture Boot IDs are visibly
named as fixture identities and are not claims about a currently running physical boot.

The pinned Netherwick profile's independent watchdog, bounded TTL/queue, stop, E-stop, bump/contact
withdrawal, cliff, wheel-drop, heartbeat, tilt, impact, and charging interlocks remain external
safety truth owned by the brainstem. Conduit cannot clear, replace, or command through them.

Run the repository proof with:

```text
cargo xtask demo netherwick --json
```

The stop line excludes motion, possession, actuator HIL, safety clearing, navigation, SLAM,
autonomy, and any parallel robotics scheduler or authority store.
