# Device mechanisms

This directory owns reusable, lower-level device and protocol mechanisms.

Existing code here describes exact device protocols, finite device-local state,
and local safety behavior. Under [#4833](https://github.com/dancxjo/conduit/issues/4833),
ordinary protocol behavior migrates to reviewed checked plots over bounded bases.
New ordinary device support should add a plot over an existing capable base.
Rust retains physics-facing primitives and non-bypassable safety; the
[responsibility audit](../../docs/architecture/device-protocol-plots.md) identifies
what remains, what migrates, and which current paths are temporary debt.
Do not remove a working Rust path before an equivalent plotted path is integrated. It does not own portable semantic kinds, plots, host or
boot composition, planning, body or application orchestration, target
make, or proof-class promotion.

The package names and their protocol identities remain stable when a crate is
filed here. Platform and application layers consume these mechanisms through
ordinary dependencies and remain responsible for authority, realization, and
execution truth.
