# Bounded spatial data

Conduit spatial data extends the frame, transform, clock, uncertainty, calibration, and provenance contracts in the spatial foundation. It does not define a world map, a SLAM engine, a ROS message identity, or an observation-to-command path.

The current range-scan, occupancy-grid, and trajectory values carry schema version and identity, normalized representation identity, immutable snapshot identity, provider and provenance identities, frame and unit, clock validity, calibration, uncertainty, and coverage. A host may publish and execute the basic transform foundation without installing any scan, point-cloud, trajectory, or map provider.

The first hosted proof is a two-chunk deterministic scan. Chunk indices and total count are checked before retention; split and coalesced representations normalize to the same scan. An exact existing `spatial/transform3` moves every point into the map frame, then a finite raster operation produces one 2 by 2 occupancy-grid snapshot. No lookup, frame conversion, map completion, or provider discovery is ambient.

The same provider exposes a two-pose deterministic trajectory fixture and bounded inspector. Its frame, clock, interpolation policy, snapshot, pose count, byte ceiling, and work ceiling remain exact until an explicit inspector projects ordinary text; the text projection does not become trajectory authority.

Current hard ceilings are eight scan points, four chunks, sixteen grid cells, eight trajectory poses, 4096 retained bytes, and 128 work units. Node ports and cords add their ordinary plan-visible value, queue, pressure, cancellation, and evidence limits.

Failures remain distinct: schema mismatch, snapshot or representation drift, provider absence, frame/unit/clock mismatch, calibration mismatch, stale transform, excessive uncertainty, resource pressure, chunk gap/reordering, partial coverage, cancellation, and wrong value type. Partial coverage never becomes a complete grid. Presentation may inspect these values but cannot supply missing coverage or mutate their identities.
