# Workspace: arriving in a Body

Product direction requested 8 September 2026. This document describes the
intended browser and ConduitOS experience; it is not an implementation or release receipt.

The front page invites, Tour teaches, Crèche births or admits machinery,
Gallery introduces activities, Workspace is home, and Patchbay inspects how
an activity happens. Arrival should let a person use a Form immediately.

Arrival without a Body shows the existing Crèche, including its naming traditions,
suggestions, and exact Form selection. Do not introduce a second birth screen or
name generator. Birth and first-Host admission lead directly into Workspace.
Tour and Patchbay should be selectable resident Forms, alongside other reviewed
activities, with truthful implementation availability on each Host.

Issue [#3152](https://github.com/dancxjo/conduit/issues/3152) adds startup sound and
generic durable first-wake behavior to this scope. A Body does not boot into a
desktop. It wakes its installed Forms. Some Forms draw; some listen; some publish;
some may simply make a sound. Workspace is an optional graphical embodiment,
not a privileged or universal startup application.

An original, bounded chime should use ordinary semantic sound contracts and
admitted audio implementations. Missing or denied audio must not block the Body's
wake. Normal startup sound is eligible per Wake; first-wake behavior has a
separate durable identity scope and ordinary Conduit State/evidence, never a
chime-specific browser flag. This requires canonical examples, deterministic
lifetime/reload proof, and a real audio Host implementation with truthful platform
permission handling.

The continuous execution foundation is owned by the parallel branch
`codex/3106-continuous-gallery-foundation`. Its kernel, sources, Gallery Form
sources, Tour runner, and associated proofs are outside this change's ownership.

## Product contract

A named Body heads a sparse surface. The foreground Form receives input through
its selected Host implementation. Morse is the first arrival: type a character,
see its admitted manifestation, and type another in the same Play. Scratch is
bounded editable text. Signals projects current evidence. Gallery offers the
reviewed Forms that can be brought into the Body's workload.

The persistent strip names the activity, its actual flow, and its current
execution state. Selecting the name opens Form information; selecting the flow
opens Patchbay; selecting the state opens lifecycle and evidence. Ordinary use
requires none of these inspections. Native controls retain keyboard and assistive
technology semantics, and only the foreground surface receives human input.

Opening a Gallery Form proposes a complete workload change, admits it through
the existing Body lifecycle, and foregrounds its surface. A refused proposal
preserves the coherent existing workload or explicitly reports Lull. Selecting
a surface does not independently create a scheduler or silently restart a Play.

## Runtime boundaries

Workspace consumes the existing Body, membership, Workset, Wake, immutable
Body Plan, and browser execution implementation. The DOM owns presentation,
never lifecycle truth. A friendly name is not a Body identity. Input focus does
not grant effect authority. No authored Form contains browser or device facts.

An awake Body and a listening Form are separate facts. Preparing, waiting for
input, quiescent, pressured, lulled, completed, cancelled, failed, and missing
evidence must remain distinguishable. A Play with an exhausted one-shot source
must not be described as listening. The continuous-source and transform work
in #3106 and #3107 is a dependency of the first live Morse surface; the full
Gallery migration and repeated-interaction proofs belong to #3109 and #3110.

The page may retain a bounded last-selected surface and drafts alongside durable
Body continuity records. Reload establishes a new Boot and requires current
membership, offers, authority, and fresh admission. It never recovers a prior
Play by displaying its saved identifier. A closed browser tab provides no
background execution guarantee. Storage refusal is visible and never converted
into successful restoration.

## Ownership and delivery proof

The browser product lives in `products/workspace/`. Host and runtime integration
belongs to `targets/browser/`; standing source and transform corrections belong
to their semantic and runtime owners. Tour, Crèche, and front-page changes are
limited to the arrival and Gallery handoffs. Existing Patchbay remains the
inspection implementation. Native ConduitOS consumes the same product meanings through its admitted
surfaces; its emulator journey is required in addition to browser proof. Physical
devices, cross-Host joining, and stable release acceptance remain separate proof.

Required proof includes two later separated Morse interactions in one Play,
focus isolation, explicit cancellation, deterministic pressure, exact Body
handoff, workload-refusal preservation, bounded restoration with fresh Boot,
accessible narrow-screen interaction, and an ordinary route from the front
page and Crèche. Visual design fixtures are explicitly labelled and establish
only surface layout. Browser acceptance uses pinned Chromium, one worker, and
zero retries; captured images follow semantic assertions.
