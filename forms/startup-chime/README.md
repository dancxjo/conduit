# Startup Chime

A Body does not boot into a desktop. It wakes its installed Forms. Some Forms
draw; some listen; some publish; some may simply make a sound.

`main.conduit` connects `body/wake` to `sound/startup-chime`. It has no graphical
Port, input device, Host binding, or audio API in its meaning. The original cue
is a deterministic 1.2-second gesture: softly enveloped 294, 441, and 588 Hz
triangle tones starting at 0, 120, and 270 ms. Exact envelopes, gains, and the
three independent reference-synth voice banks live in
[the shared renderer](../../mechanisms/implementations/synth/src/startup_chime.rs).

Try it through the shared Crèche:

```sh
cargo xtask demo workspace
```

The browser workspace selects Memory Lantern and Startup Chime for a new Body
when the browser exposes its audio API. Uncheck either Form before Birth to
omit it. Selecting only Startup Chime births a sound-only workload: it makes its
cue, settles idle in the same living Play, and stops only when lulled. A later
ordinary Wake is eligible for a new cue. The separate First Wake Chime is opt-in.
The current arrival surface does not yet edit an existing Body's workset;
Gallery installation and removal remain separate workspace work.

## Lifecycle scope

`body/wake` emits one exact true Boolean in the first admitted Play of each
Body Wake. `body/first-wake` emits it only in the first admitted Play of the
Body's first Wake. Ineligible sources close without emitting. Both are ordinary
installed sources; later observations of the same Play do not restart them.
The sink has no first-boot or already-fired flag.

Eligibility comes from ordinary retained `Woke` and `PlayStarted` (or held-Plan
release) evidence. Replacing a Plan in the same Wake does not replay either
source. Reopening the retained Body creates a fresh Boot and Wake, so the
ordinary wake source is eligible and the first-wake source is not. Adding a
new Form or placement later does not reset this **Body** scope. Rebirth with a
new Body identity starts a new lifetime; copying the same retained identity
continues its existing history.

The Host saves Started before dispatching external effects. A failed save
cancels the actual Play before playback. A crash after that durable start can
prevent an audible effect; startup evidence is not a promise of exactly-once
physical delivery. Denied or lost playback is not silently retried on a later
user gesture, replacement Plan, or first-wake reload.

## Browser realization and proof

The browser implementation prepares the canonical 57,600 signed-16 mono frames
at 48 kHz before Play, then copies them into one shared audio buffer. Each
admitted optional-cue slot permits one concurrent platform source and has a
2.5-second completion deadline. Platform source creation and teardown happen
inside the admitted audio operation; the kernel's storage limits do not grow.

A cue slot admits an attempt, not permission to make sound. Suspended audio is
reported as denied without queueing it for a later gesture. Missing output and
rendering failure are reported separately. Those outcomes complete the optional
sound sink while other Forms continue. Explicit mute policy and removal are not hidden
lifecycle flags. The adapter honors its explicit mute input; no product-wide
mute preference existed before this slice.

The lifecycle inspection contains the current exact Play's kernel events and a
separately labeled Host completion history. That history retains at most 64
accepted completions, with exact omitted counts when older observations roll
off. Denial and failure codes remain distinct from successful completion.

```sh
cargo xtask demo workspace --check
```

The pinned Chromium journey checks real audio-source start and completion,
sound-only idle, later ordinary Wake, first-wake silence after fresh-Boot reload,
and usable keyboard Forms with unavailable audio. Deterministic tests cover
exact source emissions, invalid history, replacement Play, new Body identity,
correlated denial, bounded outcome retention, and equality with the shared DSP.
Browser rendering proves browser execution; it does not measure a physical
speaker or establish native audio support.

Tracked in [#3152](https://github.com/dancxjo/conduit/issues/3152).
