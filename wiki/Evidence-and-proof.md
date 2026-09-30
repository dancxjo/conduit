# Evidence and proof

Conduit treats evidence as typed truth with a scope.

A passing test, browser run, emulator boot, physical device run, and human enactment establish different things. One does not magically upgrade into another.

## Signs

A **sign** is bounded evidence about what is true or what happened.

Signs can record:

- selection;
- preparation;
- execution;
- completion;
- abnormal termination;
- cancellation;
- effect attempts;
- host/line loss;
- replanning;
- body lifecycle transitions.

A sign is not authority. Recording "camera available" does not grant camera permission.

## Proof classes

| proof | what it establishes |
|---|---|
| deterministic contract test | identities, bounds, transitions, refusal behavior in the tested model |
| hosted execution | real implementation running in that hosted environment |
| browser execution | real browser/WASM path under the stated browser profile |
| emulator execution | freestanding image on the specified emulated machine |
| live transport | actual messages crossed the named transport/session |
| physical/HIL | named firmware/device behavior under recorded physical conditions |
| human enactment | a person actually completed the interaction |
| released-product proof | an exact accepted commit crossed the protected release boundary |

These classes can compose, but they do not substitute for one another.

## Why the distinction matters

Examples of claims Conduit deliberately refuses:

- "QEMU booted, therefore the physical laptop works."
- "The model generated narration, therefore a human heard speech."
- "Firmware compiled, therefore the Pico is attached and running it."
- "A screenshot exists, therefore it depicts the event claimed by the caption."
- "A provider failed but fallback worked, therefore the semantic endpoint failed."
- "The code contains a back, therefore the current host offers it."

## Exact identities matter

Good evidence binds the identities relevant to the claim:

```text
source document
checked form
expanded form
body
wake
host
boot
plan
play
back
line
show
sign
artifact
```

Not every event needs every identity, but evidence should not blur the ones that matter.

## Causal evidence

The causal DAG work exists because "event B happened after event A" is not enough to prove A caused B.

For recovery, useful evidence distinguishes:

```text
observation of loss
 -> invalidation
 -> fallback selection OR planning request
 -> replacement selection/refusal
 -> continued work or terminal outcome
```

A bounded causal trace should also admit truncation honestly instead of silently dropping ancestors and pretending the history is complete.

## Visual evidence

A screenshot should be correlated to the exact event it claims to illustrate.

Three Bodies explicitly learned this lesson: reusing one stale frame as birth, lull, failure, and recovery evidence is worse than omitting a frame.

See the repository's [visual evidence guide](https://github.com/dancxjo/conduit/blob/dev/docs/visual-evidence.md).

## Current product truth

The published [current-product truth surface](https://dancxjo.github.io/conduit/current-product.html) records development/release/publication identities and retained proof receipts.

It is useful precisely because "source contains capability" and "accepted product has evidence for capability" are different statements.

## How to read this Wiki

The learning pages may say a mechanism or semantic path **exists in the development tree**. That is not automatically a stable-release or physical-hardware claim.

When a page says **canonical**, it is talking about intended language/architecture law.

When it says **current tree**, it is talking about code/source present on `dev`.

When it says **planned** or **open**, it is not yet a claim of implementation.
