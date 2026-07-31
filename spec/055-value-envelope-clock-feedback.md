# Value envelope, clock conversion, and feedback contract current form

Status: implemented portable contract

Depends on: specifications 005, 006, 007, 008, 011, 022, 029, 037, and 046

## Recovery coverage delta

The recovery baseline already provides exact type identities, finite cord item
and byte capacity, port sensitivity ceilings, implementation representation
bindings, bounded distributed carriers, event correlation and causal
relations, replay/correction evidence, and deterministic rejection of every
dependency cycle.

Those contracts are reused. This specification supplies only the residual
facts that the baseline deliberately left to issue #134:

- plan authorization for optional per-value correlation, causation,
  provenance, sensitivity, representation, fragmentation, and time facts;
- exact clock-domain conversion bindings with finite validity and uncertainty;
- finite delay/state feedback boundaries that admit intentional cycles without
  relying on scheduler order.

Domain units, media time structure, coordinates, and calibration remain
domain-owned type or provider descriptors.

## Plan-owned envelope authorization

Each cord that can carry envelope metadata has exactly one
`PlanValueEnvelopePolicy`. Absence means that only the typed payload,
representation-owned handle, and existing cord accounting may cross that
cord. A policy identifies the cord and pins:

- the on-cord representation descriptor;
- maximum payload bytes, envelope bytes, fragment count, and fragment bytes;
- maximum timestamp count and the finite set of authorized clock domains;
- whether correlation, causation, and provenance references are permitted;
- the maximum sensitivity that may cross the cord.

All maxima are positive and finite. The fragment count multiplied by maximum
fragment bytes must cover the payload maximum without overflowing. Envelope
bytes are charged separately from payload bytes and are part of the exact plan
memory budget.

Optional fields stay optional. Authorization is a ceiling, never an
instruction to fabricate metadata. Unknown fields, unauthorized fields,
unlisted clocks, oversized metadata, sensitivity widening, and representation
substitution fail before the value becomes visible to a consumer.

Changing any policy fact changes plan identity.

## Runtime value envelope

A runtime envelope may contain:

- one representation-owned value handle and exact accounted payload bytes;
- zero or one value identity, correlation identity, causation identity, and
  provenance reference, as authorized;
- zero or more timestamps up to the plan ceiling, each naming its clock domain;
- exact fragment count and total fragment bytes;
- one sensitivity not greater than both endpoint and envelope ceilings.

Identifiers and provenance remain opaque bounded references. There is no
metadata map or provenance log. Local, distributed, replayed, corrected, and
retracted values preserve authorized facts exactly and drop nothing silently.
An adapter that changes representation, clock, sensitivity, or provenance
emits a new value identity and explicit derivation evidence.

## Clock domains and conversions

Timestamp comparison requires identical clock-domain identity. Cross-domain
comparison additionally requires one exact `PlanClockConversion` binding that
names:

- source and destination clock domains;
- positive rational scale numerator and denominator;
- signed offset;
- rounding policy;
- finite maximum uncertainty;
- observation authority and time basis;
- inclusive validity interval.

The conversion result is an interval, not a fabricated exact instant:

```text
converted = source * numerator / denominator + offset
result interval = converted +/- maximum uncertainty
```

Arithmetic overflow, a zero scale component, stale validity, wrong authority,
unsupported rounding, or uncertainty above the consumer's ceiling is a
deterministic rejection. Wall, monotonic, media, device, and replay clocks are
not interchangeable merely because their integers happen to match.

## Feedback admission

Every admitted dependency cycle contains at least one exact
`PlanFeedbackBoundary`. A boundary names:

- its owning node and the cord whose dependency edge it breaks;
- `delay` or `state` behavior;
- initialization policy and finite initial item/byte count;
- positive delay ticks and one authorized clock for delay behavior;
- maximum retained items and bytes;
- replay-gap, cancellation, and terminal-race policies.

`delay` requires positive delay and a clock. `state` may use zero delay but
requires an explicit initial-value or empty-state policy. Both require positive
finite retention. Removing all feedback-boundary edges must leave an acyclic
graph; otherwise planning fails.

Initialization is performed before ordinary node stepping. Cancellation
releases retained state within the selected finite cancellation bound.
Completion cannot win over an earlier cancellation or failure, and a terminal
boundary never emits a retained value afterward. Replay gaps follow the pinned
gap policy rather than repeating an old value by accident.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `CND-VEF-001` | missing, duplicate, or dangling envelope policy |
| `CND-VEF-002` | envelope, payload, timestamp, or fragment bound invalid |
| `CND-VEF-003` | unauthorized correlation, causation, provenance, clock, representation, or sensitivity |
| `CND-CLK-001` | clock conversion binding invalid, stale, ambiguous, or overflowing |
| `CND-FBK-001` | feedback boundary missing, dangling, or unbounded |
| `CND-FBK-002` | dependency cycle remains after admitted boundary edges are removed |
| `CND-FBK-003` | initialization, replay-gap, cancellation, or terminal policy invalid |

## Requirements

- **VEF-001:** Every optional envelope fact and finite ceiling is plan-visible
  and identity-affecting.
- **VEF-002:** Runtime and transport reject unauthorized or oversized metadata
  before consumer visibility.
- **VEF-003:** Correlation, causation, provenance, and sensitivity survive
  supported local, distributed, replay, and correction paths.
- **CLK-001:** Cross-domain time use requires an exact fresh conversion and
  preserves finite uncertainty.
- **FBK-001:** Every admitted cycle contains an explicit finite delay or state
  boundary.
- **FBK-002:** Initialization, retention, replay gaps, cancellation, and
  terminal races are deterministic and bounded.
- **VEF-004:** Deterministic, hosted, browser, and constrained profiles either
  implement the same contract or report the unsupported portion exactly.

## Required conformance cases

Positive fixtures cover a payload-only value, authorized correlation and
provenance, local/distributed/replayed preservation, a fresh clock conversion,
finite delay feedback, and finite state feedback.

Negative fixtures cover unbounded or oversized metadata, fragment overflow,
unknown representation, forbidden sensitivity widening, unlisted and
incomparable clocks, stale conversion, arithmetic overflow, uncertainty above
the consumer ceiling, zero or unbounded retention, a cycle without a boundary,
missing initialization, replay/correlation impersonation, cancellation during
initialization, and terminal-versus-retained-value races.

Patchbay and Tour projections consume the exact plan and execution evidence.
They do not create teaching-only envelopes, timestamps, feedback values, or
success.
