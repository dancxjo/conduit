# Observed audio capture and playback host boundaries

Issue: #147. Parent: #133. Provider, effect, lease, workload, standing-run,
and clock foundations: #137, #135, #136, #214, #257.

## Ownership and non-goals

`conduit-media` owns host-neutral audio-frame, capture, playback, and bounded
processing semantics. `conduit-audio` owns the opt-in hosted device provider.
ALSA, CoreAudio, WASAPI, WebAudio, framework callbacks, friendly device names,
and ambient default-device policy never enter media ports. A conforming host
may install no audio provider.

Capture and playback are standing boundaries, not finite record/play commands.
Editing, resolving, or selecting a device never starts a run. No universal
real-time or low-latency claim is made.

## Observation, negotiation, and exact identity

Compiled support, an initialized provider, a current device observation, a
grant, and a successful open are separate facts. Describe and resolve may
enumerate a finite closed inventory and hash provider artifacts, but must not
open a device or request permission. Unavailable, unsupported,
permission-denied, and observation-overflow states remain explicit.

An observation contains a stable direction-specific resource ID, a separate
friendly label, generation, observation and expiry ticks, backend artifact
identity, and a finite device inventory. `default`, `sysdefault`, plug,
conversion, routing, and other ambient aliases are not exact devices. The
initial hosted ALSA profile admits only observed `null` and direct `hw:`
endpoints from fixed `/usr/bin/arecord` and `/usr/bin/aplay` artifacts.

Resolution checks one current observation and seals all requested and admitted
format, rate, named layout, period, buffer, latency, sharing, concurrency,
workload, queue, work, evidence, clock, and lifecycle fields. Requested and
admitted values remain distinct even when equal. The exact plan also pins the
provider implementation, artifact digest, backend identity, resource, grant,
lease, cleanup limits, and host observation. A friendly label is presentation,
not resource identity.

Hosted latency is classified as exactly one of `enforced`, `measured`,
`observed`, or `unsupported`. The deterministic loopback is `enforced`; the
initial ALSA profile is `observed`. Neither classification implies a deadline
guarantee.

## Authority, lease, sensitivity, and use-time checks

Provider installation and device discovery do not grant use. The host policy
observer evaluates a requested exact device and emits a separate authority
decision. Source cannot construct that decision. Capture and playback have
separate actions, grants, direction-specific resources, leases, concurrency
limits, revocation grace, and cleanup deadlines. The executor rechecks grant
status, lease availability, resource binding, and expiry at use time.

Audio content is `restricted-audio`. Evidence records identities, negotiated
bounds, lifecycle changes, queue pressure, cancellation, failures, and terminal
facts; it does not silently retain unbounded captured samples.

## Lifecycle and failure taxonomy

Where the backend distinguishes them, describe, resolve, open, start, Waiting,
first sample, playback commit, drain start, drained, stop, close, and failure
are separate events. Capture commits when its first sample is delivered to the
graph. Playback commits when the backend accepts a frame. A standing patch
remains live after either commit.

Temporary absence of a ready frame registers one exact timer interest and is
Waiting, never completion. Stop, bounded drain, Abort, cancellation before
open, cancellation after open, cancellation while running, cancellation during
drain, provider loss, and failure retain different evidence and terminal
classes. Underrun, overrun, format mismatch, busy/exclusive conflict, clock
drift, discontinuity, hot unplug, provider restart, and cleanup timeout cannot
be reported as clean audio.

## Clocks and standing composition

Every endpoint names its sample clock and clock-correlation quality. The
deterministic loopback uses one exact shared 48 kHz clock. The hosted ALSA
profile reports an observed, monotonic, uncertain correlation. Drift is
rejected with evidence; discontinuity and provider loss are terminal. A future
explicit resampling provider may own drift correction, but capture or playback
must not hide it.

`examples/virtual-audio-capture.panel` and
`examples/virtual-audio-playback.panel` are isolated checked node proofs.
`examples/virtual-audio-loopback.panel` composes capture, bounded Q15 gain, and
playback through the production executor. The immutable plan epoch alternates
between ready work and nonterminal Waiting until explicit cancellation.

`conduit-audio` additionally proves the same composition against observed ALSA
`null` when that hosted backend is present. Hosts without the tools, devices,
or permission report the corresponding unavailable state instead of
fabricating support.

## Conformance and teaching

`conformance/c4/audio-device-boundaries.json` names every positive, negative,
and lifecycle fixture and its owning executable test. The selectable Tour
lesson shows both node contracts, isolated checked panels, their bounded gain
composition, exact plan fields, Patchbay topology, and an ordered
RxJS-marble-like evidence timeline. The ordered textual event table duplicates
all time, value, pressure, state, cancellation, error, and terminal facts for
keyboard, screen-reader, reduced-motion, and non-audio use.
