Native x86_64 QEMU ECM proof
===========================

Checked package SHA-256: `1a341ecaf311abeb2a6cb1618134bcf77beb8cd0a17f2def05c99d05b6e205c6`.
Kernel source: `d2ab8e8aa20a3468a41294d364ae8dbc63a61ca5`.
The checked package includes the complete canonical pair and route envelope corrections.

Run `python3 validate_ecm_current.py` in this directory to correlate all canonical transmitted frames and received bytes with the external usbredir wire transcript. The run includes one full-MTU received stimulus, a full-MTU transmitted frame, and 1460 TCP bytes in each direction through the existing network owner. It records zero Play allocations/reallocations, acknowledged Stop and normal retirement.

This is emulator evidence, not physical-device evidence or proof of every fault mode.
