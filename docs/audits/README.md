# Development audits and point-in-time investigations

Files in this directory capture named investigations, candidate comparisons,
refusal checks, and observations. They are valuable evidence **about their own
checkpoint**. Their timestamps, firmware images, host profiles, command
arguments, and measured limits must not be silently promoted into general
present-day behavior.

For a reader asking "what works today?", start with [STATUS](../../STATUS.md).
For the architecture's enduring obligations, use the
[canon](../conduit-canon.md). For retained accepted milestones and retired
designs, use [history](../history/README.md). For a target-specific proof,
consult the target's own README and the named run's exact receipt.

## Find an audit by subject

- **ConduitOS and native graphical work:** consult
  [ConduitOS protection domains](../architecture/conduitos-protection-domains.md)
  and [the target guide](../../targets/conduitos/README.md) before reading
  any dated firmware or compositor report here.
- **Browser, Face, and Mask:** begin with
  [universal Face grammar](../architecture/face-grammar-conformance.md) and
  [browser target](../../targets/browser/workspace/README.md).
- **Language, numeric, and audio:** begin with
  [checked language surface](../../wiki/Current-language-surface.md),
  [semantic Type ownership](../architecture/semantic-type-ownership.md), or
  the [speech component](../../semantics/speech/README.md).
- **Embedded/environment:** begin with the
  [environmental Body](../../bodies/environment/README.md),
  [RP2040 target](../../targets/rp2040/README.md), or
  [ESP32 target](../../targets/esp32/README.md).
- **Build, optimization, and execution:** use the
  [performance index](../performance/README.md) and
  [CI guide](../contributing/ci.md) for supported present entrances.

## How to read a result

Name the source branch/commit and hardware/host profile; distinguish
successful physical behavior from compilation, simulation, inspection,
forecast, or an explicitly failed allocation. Refusal evidence often matters
as much as a successful run. A superseded limitation remains historically true
even if a later representation or realization removes that bottleneck.

This directory is a record collection, not a second roadmap. New active
instructions belong in their owning target or topic guide. The
[documentation policy](../documentation-policy.md) explains the boundary.

For the inventory and cross-reference corrections in the 25 October review,
see the [documentation sweep record](documentation-sweep-20261025.md).
