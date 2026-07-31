# Bounded spatial foundation

Status: current pre-release contract.

This specification owns reusable frame, transform, stamped point, twist, and
calibration semantics above `conduit-core`. It does not own a robot, command,
world frame, hidden transform tree, ROS message identity, or another scheduler.

## Explicit identity and numeric profile

`SPT-001` Every spatial value MUST name its frame. Every transform MUST name
distinct source and target frames plus linear unit, handedness, axis convention,
rotation representation, clock, stamp, validity interval, uncertainty,
calibration identity, and provenance identity. A host MUST NOT infer any of
these from node names or ambient state.

`SPT-002` Clock identities are exact. Comparing values from different clocks
requires a finite, admitted clock conversion from the time foundation. Missing,
stale, or directionally wrong conversions fail closed.

`SPT-003` The first numeric profile uses checked signed micrometres, right-handed
`X-right/Y-forward/Z-up` axes, normalized Q30 quaternions, exact quarter turns
around positive Z, checked integer interpolation, and checked pinhole projection
in millipixels. Overflow, invalid quaternion/profile disagreement, singularity,
and a point behind or outside the camera are typed failures.

## Bounded operations

`SPT-004` Transform graphs, history values, interpolation windows, numeric work,
encoded values, cord occupancy, queued bytes, and evidence are finite. The first
proof admits at most 16 graph edges, two history values per interpolation, four
ticks of interpolation, 256 work units, 256 bytes per value, and capacity-one
lossless blocking cords.

`SPT-005` Compose means apply the first source-to-middle transform and then the
second middle-to-target transform. Invert swaps frames and applies the exact
inverse rotation and translated origin. Apply requires the point frame, clock,
and stamp to match the transform. No operation searches an ambient tree.

`SPT-006` Interpolation requires the same source, target, axes, clock,
calibration, and rotation at both endpoints. The requested tick MUST lie inside
the declared finite endpoint window. It never extrapolates.

`SPT-007` Projection and unprojection require the exact camera frame and
calibration identity, positive focal lengths and depth, nonzero image bounds,
and a calibration valid at the value tick. Calibration is not discovered or
corrected automatically.

## Availability, cancellation, and evidence

`SPT-008` A host MAY understand all spatial contracts while installing no
spatial provider. Contract registration is separate from provider installation;
an unsupported or stale provider is rejected during exact resolution.

`SPT-009` The ordinary exact executor owns scheduling, bounded pressure,
cancellation, and terminal evidence. Cancellation emits no successful spatial
value. Provider loss and unsupported provider remain distinct from bad spatial
input.

## Owned nodes

- `spatial/transform/literal` and `spatial/point/literal` create checked finite values.
- `spatial/transform/compose`, `spatial/transform/invert`,
  `spatial/transform/lookup`, `spatial/transform/interpolate`, and
  `spatial/transform/apply` perform canonical checked transform operations.
- `spatial/camera/project` and `spatial/camera/unproject` cross the explicit
  calibration boundary.
- `spatial/point/inspect` emits a bounded textual proof projection.

`conformance/c4/spatial-foundation.json` owns the positive and negative matrix.
`examples/spatial-transform-interpolate.panel` is the standalone bounded-time
proof. `examples/spatial-transform-compose.panel` composes transforms,
inversion, application, projection, unprojection, and an ordinary display sink.

## Non-goals

There is no universal world frame, implicit unit conversion, hidden hierarchy,
automatic calibration, observation-as-command, mandatory floating-point
accelerator, unbounded point cloud, image, map, or framework-owned spatial type.
