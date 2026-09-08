# What we're building next

This is a navigation snapshot of open work, reviewed on **8 September 2026**.
Follow the linked issue for its current state, dependencies, acceptance criteria,
and scope. An open issue is planned or unfinished work, not a claim that someone
is actively implementing it. Paused work is marked below.

Conduit's direction is one continuing Body running ordinary reusable Forms
across the machinery available to it. The near-term task is to make those
capabilities understandable and useful through real product experiences.
[Current status](../STATUS.md) describes the implementation; the
[canon](conduit-canon.md) describes the enduring design.

## ConduitOS shell

The [QEMU visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)
already shows the graphical product. Retained compositor surfaces and input
routing exist beneath it; the Tour journey now checks distinct retained
workspace/status identities. The shell series completes product proof and
improves everyday interaction:

| Work | Issue |
|---|---|
| Independently retained workspace, status, and inspection surfaces | [#3043](https://github.com/dancxjo/conduit/issues/3043) |
| Gear Back inspection on its own surface | [#3044](https://github.com/dancxjo/conduit/issues/3044) |
| Transient chooser, refusal, and confirmation surfaces | [#3045](https://github.com/dancxjo/conduit/issues/3045) |
| Explicit resize and relayout lifecycle | [#3046](https://github.com/dancxjo/conduit/issues/3046) |
| Bounded scrolling and clipping | [#3047](https://github.com/dancxjo/conduit/issues/3047) |
| Visible cursor, hover, and keyboard focus | [#3048](https://github.com/dancxjo/conduit/issues/3048) |
| Readable typography, icons, spacing, and software rendering | [#3049](https://github.com/dancxjo/conduit/issues/3049) |

The issues define their dependencies. Rendering polish follows usable surface
interaction; it should not invent new application state or runtime semantics.

## Continuous execution

[Lifecycle default #3006](https://github.com/dancxjo/conduit/issues/3006) makes
drained work quiescent by default, with semantic completion requiring an
explicit witness. Later input should resume the same admitted Play and State.
This implements the [continuous-execution contract](architecture/continuous-execution.md)
through the kernel and Host runners.

## The House and a physical laptop

[House #2293](https://github.com/dancxjo/conduit/issues/2293) is the concrete
multi-Host Body experience: the house keeps its identity as browsers, computers,
devices, and services join or leave.

| Slice | Remaining outcome |
|---|---|
| [House admission #2296](https://github.com/dancxjo/conduit/issues/2296) — **paused** | A persistent house admitting additional browser Hosts without rebirth |
| [Speech #2297](https://github.com/dancxjo/conduit/issues/2297) | Live speech → house-name detection → local Ollama response → speech/presentation; recorded-audio components do not finish this journey |
| [Matter and BLE #2298](https://github.com/dancxjo/conduit/issues/2298) | Supported local smart-home devices exposed as ordinary capabilities |
| [Google Home adapter #2299](https://github.com/dancxjo/conduit/issues/2299) | An honest adapter for supported Home/Nest devices |
| [Physical laptop #2300](https://github.com/dancxjo/conduit/issues/2300) | A real old x86_64 laptop running ConduitOS as a house Host |

The laptop campaign progresses through [boot #2301](https://github.com/dancxjo/conduit/issues/2301),
[inventory #2302](https://github.com/dancxjo/conduit/issues/2302),
[storage #2303](https://github.com/dancxjo/conduit/issues/2303),
[network #2304](https://github.com/dancxjo/conduit/issues/2304),
[audio #2305](https://github.com/dancxjo/conduit/issues/2305),
[input #2306](https://github.com/dancxjo/conduit/issues/2306),
[remaining devices #2307](https://github.com/dancxjo/conduit/issues/2307), and
[the spoken house response #2308](https://github.com/dancxjo/conduit/issues/2308).
Each physical result needs its own device evidence; the QEMU gallery does not
complete these stages.

## Reusable Forms

Forms can now act as Gears through their checked Faces. The remaining work is
to finish useful compositions and demonstrate reuse outside each namesake app:

- [Pocket Theremin #2217](https://github.com/dancxjo/conduit/issues/2217): input mapping and parameter control.
- [Constellation Telephone #2219](https://github.com/dancxjo/conduit/issues/2219): stroke capture, transport, and reconstruction.
- [Signal Garden #2221](https://github.com/dancxjo/conduit/issues/2221): observation, evolving state, and persistence.
- [Little Seismograph #2223](https://github.com/dancxjo/conduit/issues/2223): measurement windows, thresholds, plots, and bounded history.
- [Button Across the Room #2224](https://github.com/dancxjo/conduit/issues/2224): a reusable cross-Host button/indicator composition.
- [Night Radio #2225](https://github.com/dancxjo/conduit/issues/2225): routing, classification, logging, plotting, and alerts.
- [Bench #2226](https://github.com/dancxjo/conduit/issues/2226): device decoding, telemetry, annotation, sessions, and replay.

## Pete and navigation

[Pete #2229](https://github.com/dancxjo/conduit/issues/2229) is the longer-term
robotics Body. Its [memory #2231](https://github.com/dancxjo/conduit/issues/2231),
[resident workload #2233](https://github.com/dancxjo/conduit/issues/2233), and
[continuous physical capstone #2234](https://github.com/dancxjo/conduit/issues/2234)
slices are **paused**. [Navigation #2232](https://github.com/dancxjo/conduit/issues/2232)
remains open for portable goal, path, trajectory, and local-control semantics.
Existing deterministic robotics code does not finish the physical capstone.

## Contributor and release experience

The [release-train work #3038](https://github.com/dancxjo/conduit/issues/3038)
tracks finishing good releases, terminating known failures, and suppressing
superseded queues. Follow the [current CI guide](contributing/ci.md); automation
owns normal integration and release bookkeeping.

Small improvements to setup, examples, explanations, accessibility, and error
messages are useful across all these areas. The [contributor guide](../CONTRIBUTING.md)
helps you choose a place to start without taking on an entire campaign.

## Completed milestones and future ideas

The former roadmap, [R1 #361](https://github.com/dancxjo/conduit/issues/361), is
closed. Its physical Pico and dual-Line recovery evidence remains part of the
[acceptance history](history/accepted-milestones.md). Multi-Form Body scheduling
[#2062](https://github.com/dancxjo/conduit/issues/2062), Forms-as-Gears
[#2291](https://github.com/dancxjo/conduit/issues/2291), and the QEMU visual journey
[#2318](https://github.com/dancxjo/conduit/issues/2318) are also completed milestones,
not instructions to restart those projects.

The canon preserves [dormant ideas and unresolved questions](conduit-canon.md#the-idea-vault).
They are design context, not an additional backlog to undertake implicitly.
For changes since this snapshot, use the [live open issues](https://github.com/dancxjo/conduit/issues?q=is%3Aissue+is%3Aopen).
