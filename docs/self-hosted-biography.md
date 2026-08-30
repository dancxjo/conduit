# Body biography: Crèche to Patchbay

Conduit’s interactive book is not a disposable tour. It is the beginning of a Body’s own biography.

## Product rule

A new Body is created through the book. The first page births the Body, gives it a durable identity, chooses its initial program, and assigns a human-friendly mutable name. Every page after that teaches Conduit by changing that same Body.

The temporary environment that performs this guided birth is the **Crèche**. The Crèche helps create, provision, explain, and observe a new Body, but it is not part of the Body's identity and is never authoritative lifecycle state.

The Crèche is temporary by design. Its normal successor is **Patchbay**.

The product journey therefore has three phases:

1. **Guided birth in the Crèche** — early pages explain each action while the Body acquires its first Hosts, capabilities, Plans, and Plays.
2. **Graduation to Patchbay** — once the Body is independently viable, the Crèche offers to place Patchbay on the Body or finish without keeping a hosted management surface.
3. **Ongoing biography** — Patchbay, or another compatible reader later, projects readable Body history from durable evidence. The original Crèche session is no longer required.

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
- program/version changes;
- graduation from the Crèche;
- Patchbay placement, movement, or removal.

## Graduation from the Crèche

The Crèche is a bootstrap environment, not a permanent organ. Its job is to get the Body born, intelligible, and independently viable.

The decisive transition in the early biography is **graduation**. Once the Body has durable identity, durable evidence, and enough admitted capability to continue its intended work without the bootstrap environment, the Crèche offers two explicit endings.

### Host Patchbay on this Body

Place the ordinary Patchbay application using the Body's normal planning and hosting machinery. Patchbay becomes the enduring management and explanatory surface, including the Body's biography/history projection.

Patchbay is not a privileged control plane and does not become authoritative lifecycle state. It remains what its application contract already says it is: a projection over authoritative Form, Plan, Play, Body, Host, Boot, Sign, and Observatory truth.

The Crèche can then be deleted.

### Finish without hosted Patchbay

Finalize durable Body state and tear the Crèche down without placing Patchbay on the Body.

This is useful for small or purpose-built Bodies where permanently hosting a management application would waste resources or distort their intended shape.

The Crèche can be deleted completely without deleting the Body, changing its identity, or losing authoritative history. A later external Patchbay or another compatible Conduit tool can attach to the same Body and project its biography from durable evidence.

In both cases, **the Crèche ends**. The choice is whether Patchbay remains available from within the Body, not whether the birth shell is preserved.

Graduation and the Patchbay placement choice are themselves meaningful biographical events.

## Authority rule

Neither the Crèche nor Patchbay is authoritative lifecycle state. The Body remains authoritative. The biography is a readable projection.

Destroying the Crèche, closing Patchbay, or moving the Host currently realizing Patchbay must not destroy the Body or rewrite its history.

## UX direction

The book should gradually change voice:

- early: **“Next, give your Body somewhere to run.”**
- middle: **“The Pico joined and offered a physical indicator.”**
- graduation: **“Your Body can now continue without the Crèche. Would you like it to host Patchbay?”**
- later in Patchbay: **“On August 29, this Body replanned after the browser Host departed.”**

There is no hard boundary between tutorial and history. The instructional voice recedes as the Body becomes established, and Patchbay inherits the continuing biography projection after graduation.

## Migration principle

Do not preserve `tour` as the conceptual owner and merely rename headings. Move ownership toward a Body-scoped biography projection. Existing executable-book machinery can be reused inside the temporary Crèche, but durable management belongs in Patchbay and durable lifecycle truth belongs to the Body.
