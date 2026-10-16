# Acoustic control development proof

Run `cargo xtask check acoustic-control` to check the existing exact Audio
reference laws and installed Host projection evidence for #5675.

The installed tests check exact Hz/cycle round trips, explicit recognition of
unsupported logarithmic inputs, and distinct amplitude/power dB conventions.
They derive offers from checked Source, verify a Plan and execute the normal
Host kernel. Sample geometry is not substituted for a control quantity.

This is development evidence. Full reference-envelope transport through Play,
trajectory endpoint and closing-Flow partition proof, and the shared Speech
admission bridge remain outstanding. Unselected composition fixtures are drafts;
they do not establish those claims. No physical audio or calibration is claimed.
