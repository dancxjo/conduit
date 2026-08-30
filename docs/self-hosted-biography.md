# Self-hosted Body biography

Conduit’s interactive book is not a disposable tour. It is the beginning of a Body’s own biography.

## Product rule

A new Body is created through the book. The first page births the Body, gives it a durable identity, chooses its initial program, and assigns a human-friendly mutable name. Every page after that teaches Conduit by changing that same Body.

The temporary environment that performs this guided birth is the **Crèche**. The Crèche helps create, provision, explain, and observe a new Body, but it is not part of the Body's identity and is never authoritative lifecycle state.

The book therefore has three phases that share one implementation:

1. **Guided birth** — early pages in the Crèche explain each action while the Body acquires its first Hosts, capabilities, Plans, and Plays.
2. **Independence** — once the Body can continue without the Crèche, the reader chooses whether the Body adopts the biography surface or leaves the Crèche behind.
3. **Ongoing history** — whenever the biography is opened later, it renders a readable history of meaningful lifecycle events from durable Body evidence rather than depending on the original birth session.

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

## The Crèche and independence

The Crèche is a bootstrap environment, not a permanent organ. Its job is to get the Body born, intelligible, and independently viable.

The decisive transition in the early biography is therefore **independence**, not mandatory self-hosting. Once the Body has durable identity, durable evidence, and enough admitted capability to continue its intended work without the bootstrap environment, offer two explicit choices:

### Adopt the Crèche

The Body keeps the biography/management surface as one of its own services. From then on, the readable biography may be served by the Body whose life it describes.

This is useful for Bodies with enough capacity, for long-running systems, and when an always-available management surface is desirable.

### Leave the Crèche

Finalize durable Body state and tear the bootstrap environment down. The Crèche may be deleted completely without deleting the Body, changing its identity, or losing authoritative history.

This is useful for small or purpose-built Bodies where permanently hosting a documentation/management surface would waste resources or distort their intended shape.

Leaving the Crèche does **not** mean losing the biography. A later compatible Host or Conduit tool can reopen the Body and project its biography again from durable evidence. That later reader is a view onto the same Body, not a continuation of the original birth shell and not a new Body.

The choice to adopt or leave the Crèche is itself a meaningful biographical event.

## Authority rule

Neither the Crèche nor a self-hosted biography is authoritative state. The Body remains authoritative. The book is its readable projection.

Destroying a current biography renderer must therefore be semantically closer to closing a window than destroying the computer it described.

## UX direction

The book should gradually change voice:

- early: **“Next, give your Body somewhere to run.”**
- middle: **“The Pico joined and offered a physical indicator.”**
- independence: **“Your Body can now continue without the Crèche. Keep this surface with the Body, or let it go.”**
- later: **“On August 29, this Body replanned after the browser Host departed.”**

There is no hard boundary between tutorial and history. The instructional voice simply recedes as the Body becomes established.

## Migration principle

Do not preserve `tour` as the conceptual owner and merely rename headings. Move ownership toward a Body-scoped biography surface. Existing executable-book machinery can be reused, but its state model and routing should assume one durable Body whose biography can be rendered independently of the temporary Crèche that birthed it.
