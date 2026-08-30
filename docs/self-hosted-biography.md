# Body biography: Crèche to Patchbay

Conduit’s Book explains how meaning can outlive and span the machines realizing it. A separate Crèche births and provisions Bodies. A Body's biography begins with durable Body evidence, not with opening documentation.

## Product rule

A new Body is created through the independently invokable **Crèche**. The Book's opening explains the Body and birth ideas and links into that same Crèche, but the Book remains readable without creating, attaching to, or mutating a Body.

The Crèche helps create, provision, explain, and observe a new Body, but it is not part of the Body's identity and is never authoritative lifecycle state. Book launch and direct launch enter the same Crèche workflow.

The Crèche is temporary by design. Its normal successor is **Patchbay**.

The product journey therefore has four phases:

1. **Read the Book, optionally** — understand why Conduit exists and follow an explicit handoff when ready to create a Body.
2. **Guided birth in the Crèche** — the transient wizard explains each action while the Body acquires its first Hosts and capabilities.
3. **Graduation from the Crèche** — once the Body is independently viable, the Crèche may place Patchbay on the Body or finish without a hosted management surface.
4. **Ongoing biography** — Patchbay, or another compatible reader, projects readable Body history from durable evidence. Neither the Book nor original Crèche session is required.

## Narrative rule

The Book should introduce terminology only when the reader encounters the problem that terminology solves. Its opening needs only Conduit, Body, and birth. The Crèche introduces program, name, and lifecycle actions when the reader chooses to create a Body.

The reader should always be able to answer two questions:

- **What problem or Body transition is this page explaining?**
- **Why did Conduit need this concept?**

## Birth a Body

Conduit lets you build one computer out of several devices. The devices can be very different from one another: a browser, a laptop, a Raspberry Pi, a microcontroller, or anything, really.

We call a collection of devices working together as one computer a **Body**.

The Crèche birth step must:

- choose an initial program, beginning with **Morse Network** for the Crèche;
- generate a friendly default name and allow editing before birth;
- create a durable Body identity distinct from that mutable name;
- birth the Body in its initial LULLED state;
- retain that same Body independently of Book navigation or closure.

A friendly-name generator such as the Rust `petname` crate is a good fit. Generated names are labels, never Body identity.

## The biography is stateful

Crèche navigation changes transient wizard state. Book navigation changes documentation state. Neither recreates lifecycle truth. Returning to an earlier Crèche step must project the same Body and evidence that already exists; reopening the Book must not create or reset either.

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

There is a deliberate ownership boundary between documentation and history. The Book teaches; the Crèche bootstraps; Patchbay or another compatible reader projects continuing biography after graduation.

## Migration principle

Do not preserve `tour` or the Book as lifecycle owner and merely rename headings. Stateful bootstrap machinery belongs to the Crèche, durable management belongs in Patchbay, and durable lifecycle truth belongs to the Body.
