# Device mechanisms

This directory owns reusable, lower-level device and protocol mechanisms.

Code here may describe an exact device protocol, finite device-local state, and
local safety behavior. It does not own portable semantic Kinds, Forms, Host or
Boot composition, planning, Body or application orchestration, target
fabrication, or proof-class promotion.

The package names and their protocol identities remain stable when a crate is
filed here. Platform and application layers consume these mechanisms through
ordinary dependencies and remain responsible for authority, realization, and
execution truth.
