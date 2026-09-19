# body biography: Crèche to Patchbay

Conduit’s Tour explains how meaning can outlive and span the machines realizing it. A separate Crèche births and provisions bodies. A body's biography begins with durable body evidence, not with opening documentation.

This guide separates the intended product journey from its current evidence.
Tour, Crèche, durable body state, and Patchbay exist in the repository. The full
self-sufficient, multi-host biography and management journey is a design goal;
this guide does not claim it is complete on arbitrary machines. See
[status](../STATUS.md) and [the roadmap](roadmap.md) for current limits.

## Product rule

A new body is created through the independently invokable **Crèche**. The Tour's opening explains the body and birth ideas and links into that same Crèche, but the Tour remains readable without creating, attaching to, or mutating a body.

The Crèche helps create, provision, explain, and observe a new body, but it is not part of the body's identity and is never authoritative lifecycle state. Tour launch and direct launch enter the same Crèche workflow.

The Crèche is temporary by design. Its normal successor is **Patchbay**.

The product journey therefore has four phases:

1. **Read the Tour, optionally** — understand why Conduit exists and follow an explicit handoff when ready to create a body.
2. **Guided birth in the Crèche** — the transient wizard explains each action while the body acquires its first hosts and capabilities.
3. **Graduation from the Crèche** — once the body is independently viable, the Crèche may place Patchbay on the body or finish without a hosted management surface.
4. **Ongoing biography** — Patchbay, or another compatible reader, projects readable body history from durable evidence. Neither the Tour nor original Crèche session is required.

## Narrative rule

The Tour should introduce terminology only when the reader encounters the problem that terminology solves. Its opening needs only Conduit, body, and birth. The Crèche introduces program, name, and lifecycle actions when the reader chooses to create a body.

The reader should always be able to answer two questions:

- **What problem or body transition is this page explaining?**
- **Why did Conduit need this concept?**

## Birth a body

Conduit lets you build one computer out of several devices. Supported hosts can be very different: a browser, a laptop, a Raspberry Pi, or a microcontroller. Each target still needs an implementation and its own evidence.

A **body** is the durable identity of that cooperating computer. Its parts can
be realized by different hosts, and its identity survives changes in those hosts.

The Crèche birth step must:

- choose a bounded initial workload from the reviewed form inventory;
- generate a friendly default name and allow editing before birth;
- create a durable body identity distinct from that mutable name;
- birth the body in its initial LULLED state;
- retain that same body independently of Tour navigation or closure.

The current Crèche uses its deterministic persona-name catalog in
`products/creche/browser/creche-names.mjs`. Generated names are editable labels,
never body identity.

## The biography is stateful

Crèche navigation changes transient wizard state. Tour navigation changes documentation state. Neither recreates lifecycle truth. Returning to an earlier Crèche step must project the same body and evidence that already exists; reopening the Tour must not create or reset either.

The biography should eventually be derivable from durable body evidence. Guided prose may explain why an event matters, but lifecycle facts must come from the body rather than from page-local fiction.

Examples of meaningful biographical events include:

- birth and rename;
- host invitation, join, departure, and retirement;
- capability discovery or loss;
- plan creation and replacement;
- wake and lull;
- play start, completion, interruption, and recovery;
- addition of a first physical host;
- migration of work from one host to another;
- program/version changes;
- graduation from the Crèche;
- Patchbay placement, movement, or removal.

## Graduation from the Crèche

The Crèche is a bootstrap environment, not a permanent organ. Its job is to get the body born, intelligible, and independently viable.

The decisive transition in the early biography is **graduation**. Once the body has durable identity, durable evidence, and enough admitted capability to continue its intended work without the bootstrap environment, the Crèche offers two explicit endings.

### host Patchbay on this body

Place the ordinary Patchbay application using the body's normal planning and hosting machinery. Patchbay becomes the enduring management and explanatory surface, including the body's biography/history projection.

Patchbay is not a privileged control plane and does not become authoritative lifecycle state. It remains what its application contract already says it is: a projection over authoritative form, plan, play, body, host, boot, sign, and Observatory truth.

The Crèche can then be deleted.

### Finish without hosted Patchbay

Finalize durable body state and tear the Crèche down without placing Patchbay on the body.

This is useful for small or purpose-built bodies where permanently hosting a management application would waste resources or distort their intended shape.

The Crèche can be deleted completely without deleting the body, changing its identity, or losing authoritative history. A later external Patchbay or another compatible Conduit tool can attach to the same body and project its biography from durable evidence.

In both cases, **the Crèche ends**. The choice is whether Patchbay remains available from within the body, not whether the birth shell is preserved.

Graduation and the Patchbay placement choice are themselves meaningful biographical events.

## Authority rule

Neither the Crèche nor Patchbay is authoritative lifecycle state. The body remains authoritative. The biography is a readable projection.

Destroying the Crèche, closing Patchbay, or moving the host currently realizing Patchbay must not destroy the body or rewrite its history.

## UX direction

The Tour should gradually change voice:

- early: **“Next, give your body somewhere to run.”**
- middle: **“The Pico joined and offered a physical indicator.”**
- graduation: **“Your body can now continue without the Crèche. Would you like it to host Patchbay?”**
- later in Patchbay: **“On August 29, this body replanned after the browser host departed.”**

There is a deliberate ownership boundary between documentation and history. The Tour teaches; the Crèche bootstraps; Patchbay or another compatible reader projects continuing biography after graduation.

## Migration principle

Do not make Tour the lifecycle owner or mistake a heading change for a state migration. Stateful bootstrap machinery belongs to the Crèche, durable management belongs in Patchbay, and durable lifecycle truth belongs to the body.
