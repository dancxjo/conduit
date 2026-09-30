These words answer different questions.

| noun | question |
|---|---|
| **body** | what durable computer is this? |
| **part** | what durably belongs to that body? |
| **host** | what current machine/runtime can offer work? |
| **boot** | which current incarnation of that host is this? |
| **plan** | exactly how will this work be realized? |
| **play** | which active execution of that plan is happening now? |

Collapsing any two of them makes recovery and distributed execution dishonest.

## Continuity is an authored architectural fact

Traditional software often borrows identity from whatever process, VM, container, or machine happens to be alive. That is convenient until the mechanism changes.

Conduit instead gives continuity its own noun: **body**.

A body is the thing that is allowed to remain “the same computer” while hosts reboot, hosts disappear, plans are replaced, and plays begin and end. This does not mean the body is metaphysically immortal; it means continuity is represented explicitly rather than guessed from process lifetime.

That is why the nouns below are intentionally not synonyms.

## A body can outlive a host

Imagine a house body with a browser, a Linux machine, and a microcontroller.

The browser tab can close. The Linux machine can reboot. The microcontroller can disappear and return.

Those events change **presence** and **boot truth**. They do not necessarily change body identity or durable part membership.

## Birth is explicit

Birth creates the durable continuant. It does not secretly mean wake, plan, or play.

A typical lifecycle has distinct evidence:

```text
birth
 -> wake
 -> plan
 -> play
 -> lull
 -> later wake
 -> new or reused planning truth
```

A body can begin with zero, one, or many plots. No initial plot remains permanently privileged.

## Host source constructs machinery

Canonical host source is intentionally finite:

```conduit
host conduitos-native (
    target = conduitos/x86_64/pc
    build = release
    loader = limine
) {
    surface: resource presentation/surface (
        slots = 4
        bytes = 8MiB
    )

    mmio: base machine/mmio

    framebuffer: back display/linear-framebuffer (
        memory = mmio
    )

    graphics: back presentation/graphics (
        surface = surface
        display = framebuffer
    )

    policy = {
        authority: explicit,
        ambient: false
    }

    bounds = {
        heap: 16MiB,
        calls: 64,
        signs: 1024
    }
}
```

That source can help build an image. It does **not** author:

- a current HostId;
- a current BootId;
- current device instances;
- current offers;
- a current line;
- authority;
- a plan;
- a play.

Boot and runtime observation establish those facts later.

## Existing body-construction source

The current tree contains complete body-construction examples such as Pete. The present Pete profile composes several host configurations:

```conduit
body pete-r1 {
  schema = 1
  host = {
    name: "forebrain",
    part: "part:forebrain",
    configuration: "../../../targets/std/profiles/linux-computer.host.conduit",
    ...
  }
  host = {
    name: "brainstem",
    part: "part:brainstem",
    configuration: "../../../targets/rp2040/profiles/pico-w.host.conduit",
    ...
  }
  host = {
    name: "eyes",
    configuration: "../../../targets/browser/profiles/browser-page.host.conduit",
    ...
  }
}
```

See [bodies/pete/profiles/pete-r1.body.conduit](https://github.com/dancxjo/conduit/blob/dev/bodies/pete/profiles/pete-r1.body.conduit).

This is construction intent. Current membership, presence, offers, lines, plans, and plays remain runtime truth.

## Plots belong to the body; realizations belong to plans

A resident **plot** is authored intent. It may survive changes in the machinery that currently realizes it.

When a host disappears, Conduit does not need to rewrite the plot into a new machine-specific program. It can ask a narrower question: **given the same plot and the new current truth, is there another admissible plan?**

That separation is the architectural basis for recovery without semantic drift.

## Planning is where meaning meets reality

The planner starts from a checked plot and current realization truth.

```text
checked semantic requirement
        +
current host offers/resources/authority/lines
        ↓
semantically eligible candidates
        ↓
policy
        ↓
exact immutable plan
```

A plan may bind:

- a gear to an exact host and boot;
- an exact back;
- an exact implementation/artifact;
- bases and resources;
- authority;
- lines;
- queue bounds;
- expected signs;
- execution limits.

The plan is history once created. Important change produces a replacement plan rather than editing the old one in place.

## Same-plan fallback versus replan

These are not synonyms.

### Same-plan fallback

If Plan P already sealed two exact alternatives:

```text
Plan P
  provider A
  provider B
```

and A becomes unavailable, B may be selected under the sealed fallback law without changing PlanId.

### Replacement planning

If no remaining admitted realization can satisfy the semantic obligation:

```text
invalidating observation
 -> planning requested
 -> fresh current truth
 -> Plan P2 or refusal
 -> fresh play where required
```

The old plan remains immutable evidence.

The distributed Two Ollamas vertical exists largely to prove this distinction end to end. See [#4092](https://github.com/dancxjo/conduit/issues/4092).

## A play is active execution

A play owns execution history for one exact plan. The same plan may be played more than once.

During play, the kernel does not quietly rediscover the graph or acquire ambient authority. New realization decisions belong to explicit fallback already admitted by the plan or to a new planning pass.

## Why this matters

Without these identity boundaries, phrases such as "the computer recovered" become ambiguous.

Conduit wants to answer precisely:

- Was it the same body?
- Same part?
- Same host?
- Fresh boot?
- Same plan?
- Same play?
- Same line?
- Same semantic obligation?
- Was work replayed, moved, or merely continued?

The architecture makes those questions representable instead of reconstructing them from logs after the fact.
