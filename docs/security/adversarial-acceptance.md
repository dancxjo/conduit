# Adversarial security acceptance

Status: permanent executable suite for #3079.

Run `cargo xtask check security-acceptance`. The command treats the component
under test as hostile and invokes the actual trusted boundaries from the
confined Gear, Base capability, Linux process-isolated Base, federation, ROS 2,
consequential-effect, and ConduitOS profiles. Every denial family has a positive
control through the same boundary and an observer outside the hostile
principal.

The finite machine-readable matrix is
[`proof/security/acceptance-matrix.json`](../../proof/security/acceptance-matrix.json).
It records principal, legitimate scope, attack, trusted boundary, expected
disposition, independent observer, unrelated-work survival, proof class, and
the public `cargo xtask` entrance. Bearers and issuer/private keys are forbidden
from the evidence format.

The cross-Base fixture runs the process-isolated file Base while a network
sentinel is live. A hostile file provider cannot create a socket and the
sentinel independently receives no connection; the authorized network family
connects while the file Base is present and again after that provider exits.
This proves the claimed Linux file-provider blast radius and unrelated Base
survival without claiming that every native network implementation is itself
process-confined.

Proof classes remain distinct. The suite includes deterministic model tests,
Linux kernel process isolation, authenticated loopback federation, native ROS
2 Jazzy transport, QEMU x86_64 protection-domain execution, and an attended
low-energy physical-effect HIL. The physical profile remains a separate,
explicitly attended command because the deterministic acceptance suite cannot
manufacture fresh operator approval or silently repeat a physical effect.
