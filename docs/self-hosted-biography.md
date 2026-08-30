# Self-hosted Body biography

Conduit’s interactive book is not a disposable tour. It is the beginning of a Body’s own biography.

## Product rule

A new Body is created through the book. The first page births the Body, gives it a durable identity, chooses its initial program, and assigns a human-friendly mutable name. Every page after that teaches Conduit by changing that same Body.

The book therefore has three phases that share one implementation:

1. **Guided birth** — early pages explain each action while the Body acquires its first Hosts, capabilities, Plans, and Plays.
2. **Self-hosting** — once the Body can host the book itself, the biography is served by the Body whose history it records.
3. **Ongoing history** — after the guided material is complete, the same book continues as a readable history of meaningful lifecycle events rather than ending as a completed tutorial.

## Narrative rule

The book should introduce terminology only when the Body encounters the problem that terminology solves. Step 0 needs only Conduit, program, Body, name, and birth. Hosts, Boots, capabilities, implementations, Plans, Plays, Signs, wake/lull, and other concepts are introduced as the Body acquires them.

The reader should always be able to answer two questions:

- **What changed in this Body on this page?**
- **Why did Conduit need this concept?**

## Step 0: Birth a Body

Conduit lets you build one computer out of several devices. The devices can be very different from one another: a browser, a laptop, a Raspberry Pi, a microcontroller, or anything, really.

We call a collection of devices working together as one computer a **Body**.

The birth page must:

- choose an initial program, beginning with **Morse Network** for the book;
- generate a friendly default name and allow editing before birth;
- create a durable Body identity distinct from that mutable name;
- birth the Body in its initial LULLED state;
- carry that same Body through every subsequent page.

A friendly-name generator such as the Rust `petname` crate is a good fit. Generated names are labels, never Body identity.

## The biography is stateful

Navigation changes presentation state. It does not recreate lifecycle truth. Returning to an earlier page must project the same Body and the same evidence that already exists.

The biography should eventually be derivable from durable Body evidence. Guided prose may explain why an event matters, but lifecycle facts must come from the Body rather than from page-local fiction.

Examples of meaningful biographical events include:

- birth and rename;
- Host invitation, join, departure, and retirement;
- capability discovery or loss;
- Plan creation and replacement;
- wake and lull;
- Play start, completion, interruption, and recovery;
- addition of a first physical Host;
- migration of work from one Host to another;
- program/version changes.

## Self-hosting milestone

The decisive transition in the early biography is when the Body becomes capable of serving its own book. From then on, the management and explanatory surface is part of the computer being described, not an external tutorial wrapper.

This does not mean the prose itself is authoritative state. The Body remains authoritative. The book is its readable projection.

## UX direction

The book should gradually change voice:

- early: **“Next, give your Body somewhere to run.”**
- middle: **“The Pico joined and offered a physical indicator.”**
- later: **“On August 29, this Body replanned after the browser Host departed.”**

There is no hard boundary between tutorial and history. The instructional voice simply recedes as the Body becomes established.

## Migration principle

Do not preserve `tour` as the conceptual owner and merely rename headings. Move ownership toward a Body-scoped biography surface. Existing executable-book machinery can be reused, but its state model and routing should assume one durable Body whose biography continues beyond the final guided page.
