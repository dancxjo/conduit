# Standing clocked signal graphs

Status: implemented host-neutral contract catalog with deterministic reference subset

Depends on: specifications 005, 006, 007, 008, 011, 022, 029, 037, 055,
058, 066, 072, and 075

## Boundary and identities

This specification defines the semantic panels used to author modular-synth,
mixing-console, drum-machine, and signal-lab graphs. SoX, FFmpeg, Web Audio,
AudioWorklet, native DSP libraries, deterministic fixtures, and embedded DSP
providers may implement compatible subsets. None defines the graph language.

Source text, the resolved exact plan, a live run epoch, run evidence, and
Patchbay presentation remain distinct identities. Editing or connecting a cord
does not start work. Start seals one immutable plan epoch. A typed live-control
port may change an admitted parameter inside that epoch; changing pre-start
configuration or topology authors a different source and requires a new plan.

## Signal taxonomy

The current taxonomy uses five non-interchangeable contracts:

| Contract | Meaning | Required exact facts |
| --- | --- | --- |
| `conduit.media/event` | One discrete ordered occurrence | event tick, occurrence identity/order, explicit kind or selection state, delivery and pressure policy |
| `conduit.media/gate` | Held activation state | current boolean state, transition identity, transition tick, startup state |
| `conduit.media/control` | Typed time-varying level | sample tick, bounded level, lane/unit identity, discontinuity policy |
| `conduit.media/audio-frame` | Bounded timestamped PCM frame | specification 066 media time and specification 075 format/layout/frame bounds |
| `conduit.media/retained-state` | Finite temporal memory snapshot | observation tick, retained item and byte counts, bounded current value, initialization and terminal policy |

The reference binary spellings `CME0`, `CMG0`, `CMC0`, and `CMS0` are provider
representations, not the semantic type names. `CME0` has a published occurrence
kind (`trigger`, `release`, or `selection-miss`); the numeric representation tag
is not the graph meaning. A divider's unselected input becomes an explicit
ordered selection decision, not a false trigger or unexplained numeric value.
Gates are not flattened into events, controls are not gates, and none of these
ports is generic stdin/stdout.

## Time and pulse contracts

`conduit.media/time/clock` has optional typed `reset` event, `enable` gate, and
`rate` control inputs and separately typed `pulse`, `phase`, `rate`, `enabled`,
and retained `state` outputs. Its immutable configuration names:

- the time basis and positive finite period;
- startup phase and reset phase;
- the typed rate-to-period mapping and enable/disable phase behavior;
- drift behavior (`none` for the deterministic reference subset or a named,
  bounded correction contract for another provider);
- missed-pulse behavior (`coalesce`, `drop-with-count`, `fail`, or another
  published exact policy);
- discontinuity policy, maximum pending occurrences, and output pressure.

Reset takes effect at its event tick. Disable holds the published enabled gate
and suppresses later pulse selections without erasing phase unless the
contract says reset-on-disable. A live rate input is an admitted typed control;
it never edits the authored period field. Discontinuities and rate changes must
identify the first affected tick.

`conduit.media/time/timer`, `time/counter`, `time/phase-source`, and
`control/clock-divider` publish the same time basis. Counter wrap, divisor
phase, timer repetition, maximum pending occurrences, drift, missed events,
and slow-consumer pressure are finite plan facts. The deterministic reference
divider accepts divisors 1 through 1024 and an explicit phase below the divisor.

## Modulation, sequencing, and switching

The current contract catalog contains LFO, ramp, four-segment envelope, slew,
sample-and-hold, quantizer, comparator, finite-pattern sequencer, gate latch,
trigger adapter, clocked switch, crossfade, deterministic counter selection,
control tee/merge/mixer, and modulation depth/bias panels.

LFO range, period, phase, waveform, and discontinuity behavior are immutable
plan facts; its output is a control. Envelope initial/peak/sustain levels,
attack/decay/release ticks, four-segment ceiling, and
`restart-from-current` retrigger policy are finite. A sequencer declares its
finite pattern and maximum steps; repeat behavior follows the incoming selected
clock occurrences under explicit `repeat` or `stop-at-end` policy and never
creates a hidden loop. Sample-and-hold has separate
control and trigger ports and publishes `emit-initial` before its first selected
trigger. Its held value and byte/item use are visible on retained-state output.

Typed control and audio tees are executable graph elements. Their outputs are
coupled to declared bounded cords, so fan-out and pressure remain part of the
plan. They are not interchangeable with Watch.

## Mixing and live control

Specification 075 owns exact finite/standing audio mix, gain, channel mapping,
resampling, trim, and metering. This specification adds
`conduit.media/audio/controlled-gain`: timestamped audio enters on `frame` and
an admitted time-varying control enters on `gain`. The reference mapping is
explicitly `unipolar-0-1024-to-q15-0-32768`; frame/work bounds and the bit-exact
PCM numeric profile are immutable plan facts.

Control mixing, depth/bias, crossfade, pan/balance, and channel matrices keep
their numerator, denominator, saturation, layout, and maximum work visible.
No live control may add/remove a node or cord, replace a provider, or alter the
plan identity.

## Memory, delay, and recurrence

Register, one-tick delay, bounded control/audio delay line,
accumulator/integrator, bounded history, and explicit feedback-boundary
contracts publish initialization, maximum items, maximum bytes, time/delay,
gap, saturation, cancellation, drain, flush, and terminal behavior.

The plan must contain a `PlanFeedbackBoundary` for every cord whose removal
makes an admitted cycle acyclic. A state boundary has positive finite item and
byte retention. A delay boundary additionally has a positive delay and named
clock. The scheduler reserves that memory before Start. Cancellation and
terminal transitions release or drain it exactly as the plan states.

A zero-delay combinational cycle has no such boundary. It remains an authored
Patchbay topology with diagnostic `CND-CMP-001` before compilation and
`CND-FBK-002` at exact-plan admission; scheduler wake order may not resolve it.

## Standing execution and observation

An open-ended clock, capture, playback, or other live source keeps the same run
epoch alive. If no node is ready, the state is `Waiting`. Waiting is nonterminal
and distinct from successful completion, cancellation, disconnection, failure,
and a dependency-cycle diagnostic. A host resumes only through an admitted
timer or operation wake. A run ends only through natural terminal behavior
where the contract permits it, explicit Drain/Abort, failure, disconnection, or
another explicit lifecycle transition.

Scope/waveform, level/control meter, spectrum, and event-log contracts have
finite history, byte, cadence, and retention fields. Provider absence for an
optional spectrum contract is `contract-only`/unsupported, not fabricated
output. Exact Watch is separately admitted instrumentation. Attaching,
detaching, or reading Watch preserves the source hash, plan identity, run ID,
epoch, queues, pressure, timer deadline, delivery, and output. A lossless
recorder or semantic tee is a plan-visible executable element and may exert its
declared pressure.

## Deterministic reference subset and provider plurality

The production hosted executor checks event-from-ticker, divider, event/control
tees, finite sequencer, LFO, four-segment envelope, slew, sample-and-hold,
control merge/mixer/register/scope, and controlled PCM gain. Other catalog
entries may remain contract-only on a host. Availability never grants device
authority or starts a run.

The same executor also provides one deterministic virtual capture and playback
pair. Their virtual device identities, shared 48 kHz sample clock, period,
buffer, underrun, discontinuity, source-loss, drain, frame, and work bounds are
sealed inputs. They prove standing capture to bounded gain to playback without
claiming a physical device, permission, real-time latency, or ambient default.

Provider conformance is by semantic contract and exact profile, not by
implementation name. The LFO reference profile is exercised with two distinct
implementation/artifact identities and byte-exact output. Optimized or host
DSP providers may satisfy the same contract only when their declared numeric,
timing, discontinuity, pressure, and terminal profile is compatible.

## Required proofs and fixtures

`conformance/c4/standing-signals.json` names every required positive and
negative case. The production proof panel
`examples/standing-signal-lab.panel` runs pulse to LFO to typed control tee,
control-to-audio, live controlled gain, and meter. It reaches observable
Waiting, resumes on the same exact epoch, preserves identity across Watch
attach/detach, and terminates only after explicit Abort.

`examples/clocked-sample-hold.panel` proves pulse, divider, sequencer, distinct
control/trigger ports, an exact pre-trigger initial value, retained state, and
scope text. `examples/bounded-control-feedback.panel` seals a finite
`PlanFeedbackBoundary`; `examples/invalid-zero-delay-feedback.panel` preserves
the invalid authored cycle for Patchbay and fails with the precise cycle
diagnostic. `examples/virtual-audio-loopback.panel` runs capture through one
bounded gain stage into playback, repeatedly reaches nonterminal Waiting, and
stops only through explicit lifecycle cancellation.

Each fixture entry names the exact Rust runner that owns its assertion. The
current conformance set covers startup phase; reset while running;
enable/disable; rate change; missed pulse; slow consumer; clock drift;
discontinuity; counter wrap; finite versus repeating sequence; envelope
retrigger; sample-and-hold before first trigger; delay flush; bounded feedback
saturation; zero-delay cycle; cancellation during retained state; source loss;
Waiting versus deadlock; Watch attached/detached; and provider unsupported.

## Presentation and teaching

Patchbay derives faceplate and cord presentation from the exact port contract:
audio, control, gate, event, and retained-state ports have distinct color,
shape/rhythm, and text labels. Color is never the sole distinction. Invalid
recurrence stays visible with its diagnostic. Reduced-motion presentation uses
static state changes and the ordered event/value table; keyboard users can
reach the same controls.

Tour begins with a pulse generator and builds a standing patch. It separately
shows pulse/event timeline, changing control, audio frames, retained state,
feedback, and equivalent ordered text. It explicitly contrasts an imperative
loop: the graph remains present and live; pulses advance state; cords carry
typed values and occurrences; explicit memory gives feedback temporal meaning;
and lifecycle control starts and stops the immutable epoch.
