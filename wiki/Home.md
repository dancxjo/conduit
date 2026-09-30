**One continuing computer, made from the computers you have.**

Conduit is a programming system built around one durable idea: **say what the work means before deciding which present machine will perform it**.

A Conduit **body** may live on one machine or span browsers, servers, microcontrollers, robots, and other hosts. Authored meaning stays portable; planning examines the machinery and authority that actually exist; execution follows one exact admitted plan.

> **Meaning stays portable because realization stays exact.**

If you want the philosophy before the syntax, begin with [[Why Conduit|Why-Conduit]].

## The five-minute mental model

Learn the nouns in small groups; they are deliberately not one giant hierarchy.

| cluster | vocabulary | distinction |
|---|---|---|
| information | **type / form** | what a value means / one concrete portable representation |
| semantic work | **kind / fore / back** | what work means / its user-visible callable signature / one realization |
| authored execution | **plot / plan / play** | intended happening / admitted realization / realization in motion |
| graph realization | **gear / port / cord / line** | occurrence / endpoint / semantic connection / concrete carriage |
| continuity | **body / part / host / boot / wake / lull** | enduring computer / membership / machinery / incarnation / activity / repose |
| time and storage | **keep / data** | retained current truth / independently addressable immutable generation |
| human encounter | **face / mask / show** | human meaning / user-agent realization / concrete manifestation |
| evidence | **sign** | bounded evidence about what was true or happened |

The words matter because each boundary prevents one kind of mechanism from quietly becoming another kind of meaning.

## Plot, plan, play

The simplest architectural story is:

```text
user-authored plot
      +
current body, hosts, backs, resources, authority, lines
      ↓
     plan
      ↓
     play
```

A **plot** says what should happen. A **plan** says exactly how the current world can make it happen. A **play** is that plan happening.

If a host disappears, the plot need not change; if another eligible back exists, planning can produce a new realization without pretending the semantic work itself changed.

## Meaning has forms; work has backs

Information and work each separate semantics from realization:

```text
type  → form
kind  → back
```

A **type** says what a value means; a **form** says how values of that type may be carried or stored.

A **kind** says what work means; a **back** is one concrete way to realize it. The **fore** is the function-like signature presented to the author: the startup parameters and typed ports through which that kind or plot is called.

This symmetry is central to Conduit: compatibility facts, machine layouts, libraries, devices, and transports should not become semantic identity merely because they were convenient first implementations.

## A tiny authored plot

The canonical vocabulary is moving from historical `form` to **plot**, while **form** is being reassigned to portable type representation. During that migration, checked-in source may temporarily use the older spelling; [[Current language surface|Current-language-surface]] records the exact implemented grammar.

Conceptually:

```conduit
plot hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." >> upper >> show
}.
```

The plot asks for uppercase text and presentation. It does **not** say Linux, browser, stdout, framebuffer, WebSocket, process, or CPU. Those are realization facts.

## Why this is more than portability

Conduit is not trying to erase the machine.

It wants the machine to be **more visible at the right layer**:

- source owns semantic intent;
- hosts truthfully describe what they can offer;
- planning makes exact choices;
- plays execute only those choices;
- signs record bounded evidence;
- replacement planning happens when reality invalidates the old realization.

The same discipline appears elsewhere: cords are not lines, bodies are not hosts, keeps are not saved data, faces are not shows, evidence is not authority, and quiescence is not completion.

## What is Conduitese?

**Conduitese is Conduit's human-authored source language.** It describes portable semantic plots, semantic types and their forms, and construction truth without granting portable source an escape hatch into arbitrary host APIs.

It is graph-oriented, typed, temporally explicit, resource-conscious, and designed so important effects, bounds, state, terminals, and relationships remain inspectable.

Read [[Conduitese]] for the language model, then [[Conduitese by example|Conduitese-by-example]] for annotated source.

## Suggested reading path

1. [[Why Conduit|Why-Conduit]] — the enduring architectural wager.
2. [[Architecture tour|Architecture-tour]] — follow meaning into planning and execution.
3. [[Conduitese]] — how authored source expresses that meaning.
4. [[Bodies, hosts, plans and plays|Bodies-hosts-plans-and-plays]] — continuity and realization.
5. [[State, time and data|State-time-and-data]] — temporal meaning, keep, save, load.
6. [[Lines, networking and replanning|Lines-networking-and-replanning]] — semantic connection versus carriage.
7. [[Face, Mask and Show|Face-Mask-and-Show]] — human meaning versus rendering.
8. [[Evidence and proof|Evidence-and-proof]] — what observations actually establish.

## Reference

The learning pages explain the ideas; these pages define the exact current contracts:

- [[Current language surface|Current-language-surface]]
- [[Architecture reference|Architecture]]
- [[Forms and flow reference|Forms-and-flow]]
- [[Types and state reference|Types-and-state]]
- [[Terminals and concurrency reference|Terminals-and-concurrency]]
- [[Effects and realization reference|Effects-and-realization]]
- [[Packs, hosts and bodies reference|Packs-hosts-and-bodies]]
- [[Canon governance|Canon-governance]]

Conduit is experimental software. Hosted proof, browser proof, emulator proof, physical-hardware proof, and released-product proof deliberately remain different claims.
