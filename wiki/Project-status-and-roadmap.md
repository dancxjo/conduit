# Project status and roadmap

Conduit is experimental, but it is far beyond a paper architecture.

The current development tree includes checked `.conduit` source, immutable planning, one bounded kernel, hosted/browser/ConduitOS execution, body lifecycle, lines, Patchbay, Face/Mask/Show work, physical-device paths, and substantial evidence machinery.

This page is a navigation map, not a replacement for [STATUS.md](https://github.com/dancxjo/conduit/blob/dev/STATUS.md) or the live issues.

## What exists in the development tree

Broadly:

- checked Conduitese forms with typed ports and finite bounds;
- forms as reusable semantic gears;
- immutable plans and bounded play execution;
- host calls, resources, authority admission, and pressure;
- normal close, typed abnormal terminal truth, and cancellation distinctions;
- body birth/wake/lull/workset behavior;
- Face composition plus graphical/browser/spoken mask machinery;
- Patchbay inspection and bounded causal/replay surfaces;
- hosted, browser, and ConduitOS host families;
- bounded remote lines and rendezvous paths;
- physical Pico work;
- ConduitOS graphical x86_64 plus four serial product architectures;
- deterministic and environment-specific proof lanes.

See [[Evidence and proof|Evidence-and-proof]] for what those statements do and do not establish.

## The current language campaign: Conduitese v1 authors itself

The major current language epic is [#4375](https://github.com/dancxjo/conduit/issues/4375).

Its core symmetry is:

> **type : info :: kind : gear**

Recent progress includes native authored semantic types and generated Rust bindings. The merged LinguisticOffsetBasis migration is a concrete example.

Still-live work includes:

- [#4378](https://github.com/dancxjo/conduit/issues/4378) — bounded sequence/flow algebra instead of imperative loops;
- [#4382](https://github.com/dancxjo/conduit/issues/4382) — migrate portable semantic type ownership out of handwritten Rust;
- [#4399](https://github.com/dancxjo/conduit/issues/4399) — bounded activation of one statically selected subgraph.

The design line remains strict: no general loop runtime, runtime closures, or ambient semantic escape hatch just to make authoring convenient.

## The active verticals

These are especially useful because they force several architectural layers to meet.

### Durable Notebook

[#4116](https://github.com/dancxjo/conduit/issues/4116)

Proves the difference among:

- current retained truth;
- exact keep lifetime;
- explicit sampling;
- immutable saved data generations;
- typed load;
- recovery after hard loss.

Guiding sentence:

> **keep says how long current truth must live. save says make this particular truth independently addressable.**

### Masks and wardrobe

[#4115](https://github.com/dancxjo/conduit/issues/4115)

Makes Mask an ordinary form role, with authored `wear`, `doff`, `want`, and `else` policy.

### Host: bare metal to show

[#4117](https://github.com/dancxjo/conduit/issues/4117)

Connects host source to build/image/boot/resources/backs/masks without confusing fabricated machinery with current runtime truth.

### Two Ollamas

[#4092](https://github.com/dancxjo/conduit/issues/4092)

Proves same-plan fallback versus true replan with two substitutable model providers, without teaching the scheduler "Ollama."

### Pocket Theremin

[#4091](https://github.com/dancxjo/conduit/issues/4091)

A small but severe integration test for units, Current state, bounded PCM, pressure, cancellation, and materially different audio realizations.

### Three Bodies

[#4093](https://github.com/dancxjo/conduit/issues/4093)

Three independent births:

```text
ConduitOS graphical
Browser DOM/WASM
Screen-free speech
```

Each performs useful work, encounters real loss/refusal, recovers truthfully where possible, and rests. The comparison is semantic portability, not identical pixels, prose, or timing.

### Face conformance

[#4167](https://github.com/dancxjo/conduit/issues/4167)

Pressure-tests Face against radically different encounters to keep it a human-semantic waist instead of growing into a widget vocabulary.

## Product/hardware campaigns

The repository roadmap also tracks:

- ConduitOS shell usability and retained surfaces;
- House as a persistent multi-host body;
- live speech and smart-home adapters;
- a physical old laptop ConduitOS campaign;
- reusable forms such as Signal Garden, Night Radio, Little Seismograph, and Button Across the Room;
- Pete, the longer-running robotics body.

Some of those campaigns are explicitly paused. An open issue means planned or unfinished work, not necessarily active implementation.

## ConduitOS physical ladder

The physical laptop work is intentionally staged:

```text
boot
 -> inventory
 -> storage
 -> network
 -> audio
 -> input
 -> remaining devices
 -> house/spoken integration
```

QEMU evidence does not skip those stages.

## Where the planning truth lives

For current scope, use:

1. [live open issues](https://github.com/dancxjo/conduit/issues?q=is%3Aissue+is%3Aopen);
2. [docs/roadmap.md](https://github.com/dancxjo/conduit/blob/dev/docs/roadmap.md) for the larger campaign map;
3. [STATUS.md](https://github.com/dancxjo/conduit/blob/dev/STATUS.md) for capability/proof boundaries;
4. this Wiki for the concepts and language.

The roadmap document is a dated snapshot. Issue bodies and current code may have moved since its review date.
