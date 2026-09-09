# Consequential physical effects

Status: generic provider-gate contract, deterministic conformance, and an
attended low-energy physical-HIL profile for issue #3100.

`ConsequentialEffectGate` is the reusable last trusted seam for realizations
which can affect people, property, or the physical environment. Consequence is
a realization profile fact: the same portable meaning may select an ordinary
indicator, a controlled bench fixture, or an attended hazardous provider
without changing authored Form meaning.

The gate admits an effect only when all of these independently agree:

- exact current resource identity and generation;
- an unforgeable, Play-scoped #3072 Base capability;
- a fresh, single-use attended possession when the profile requires it;
- current local safety/interlock evidence;
- operation-specific magnitude, duration, and rate bounds.

The provider returns `Observed` or `Uncertain`; command submission is never
reported as physical success. An ambiguous provider result terminates the
attempt and is not retried. Provider/Line/Host loss revokes the capability and
invokes the profile's explicit local safe disposition. Failure of that cleanup
remains separately visible.

The attended issuer, capability table, local safety observer, and final effect
provider are separate trusted boundaries. A planner, remote peer, model, Body
membership, valid transport, or stale prior approval cannot manufacture their
possessions. ROS, Pete, smart-home, GPIO, industrial, and other Bases should
consume this contract and add their domain envelope rather than creating a
parallel authority system.

Run `cargo xtask check consequential-effect` for deterministic conformance.
That suite uses an observable low-energy fixture model and proves that missing,
expired, forged, stale, unsafe, and out-of-envelope inputs never reach its
provider; it does **not** make a physical-HIL claim. Physical acceptance must
use an attended device and independent observation, and record that separate
proof class honestly.

The physical profile is `cargo xtask check physical-effect-hil`. It requires
the exact `CONDUIT_PHYSICAL_INTERFACE` and `CONDUIT_PHYSICAL_BROADCAST`
realization facts and then prompts for a fresh phrase naming both plus a
run-specific nonce. It sends one 40-byte UDP broadcast with no retry through
the real interface. A separate
socket observes the exact nonce-bearing payload and the kernel NIC transmit
counter must advance under bounded observation polling that never repeats the
effect. Missing attendance, an excessive envelope, and a revoked
capability are proved not to invoke the provider; provider loss reaches the
explicit no-further-transmission disposition. This is deliberately a
low-energy network fixture, not a claim about motion, mains power, locks, or
other hazardous devices.
