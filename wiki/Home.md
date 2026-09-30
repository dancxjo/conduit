# Conduit

**One continuing computer, made from the computers you have.**

Conduit is a programming system for describing **what work means** separately from **which machine happens to perform it**.

A Conduit **body** can live on one computer or span a laptop, browser, server, microcontroller, robot, and other hosts. You write portable semantic work. Conduit checks it, looks at the machinery that is actually available, chooses an exact realization, and runs that realization through one execution model.

```conduit
form hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." >> upper >> show
}.
```

That form asks for uppercase text and presentation. It does **not** say Linux, browser, framebuffer, stdout, WebSocket, process, device path, or CPU. Those are realization facts.

The compact rule is:

> **Meaning stays portable. Realization stays exact.**

## The five-minute mental model

| idea | what it means |
|---|---|
| **form** | portable semantic work |
| **kind / fore / gear** | what an operation means, how it is called, and one configured occurrence |
| **host / back / base** | current machinery and concrete realizations |
| **plan / play** | one exact immutable realization, then one active execution of it |
| **body** | the durable computer that can continue while hosts, boots, plans, and plays change |

Start with [[Architecture tour|Architecture-tour]] if those nouns are new.

## What is Conduitese?

**Conduitese is Conduit's human-authored source language.** It is the language in `.conduit` files.

It is declarative, typed, finite by construction, graph-oriented, and designed to keep semantic meaning visible. A form says how typed information moves through semantic work. Conduitese also has source roles for host construction, body construction, packs, and increasingly the semantic types themselves.

Conduitese is **not** a second operating system kernel, a shell language, a replacement for Rust, or a place to smuggle host APIs into portable programs. Rust still implements the checker, planner, kernel, host adapters, mechanisms, and many realizations. Conduitese owns the portable authored meaning that those layers consume.

Read [[Conduitese]] for the language model, then [[Conduitese by example|Conduitese-by-example]] for a tour through real `.conduit` programs.

## Explore

- [[Start here|Start-here]] — run Hello, Tour, Patchbay, or ConduitOS.
- [[Conduitese]] — what the language is and why it looks the way it does.
- [[Conduitese by example|Conduitese-by-example]] — many annotated source examples.
- [[Architecture tour|Architecture-tour]] — forms, hosts, planning, execution, bodies, lines, and evidence.
- [[Bodies, hosts, plans and plays|Bodies-hosts-plans-and-plays]] — the realization/lifecycle model.
- [[State, time and data|State-time-and-data]] — keep, Current, flows, completion, sampling, save/load.
- [[Face, Mask and Show|Face-Mask-and-Show]] — Conduit's human-interface architecture.
- [[Lines, networking and replanning|Lines-networking-and-replanning]] — distributed execution without hiding transport truth.
- [[ConduitOS and real machines|ConduitOS-and-real-machines]] — freestanding Conduit and hardware boundaries.
- [[Evidence and proof|Evidence-and-proof]] — what different demonstrations actually establish.
- [[Project status and roadmap|Project-status-and-roadmap]] — current capabilities and live verticals.
- [[Glossary]] — one-page vocabulary map.

## Exact reference

The learning pages explain. The reference pages define.

- [[Current language surface|Current-language-surface]]
- [[Architecture reference|Architecture]]
- [[Forms and flow reference|Forms-and-flow]]
- [[Types and state reference|Types-and-state]]
- [[Terminals and concurrency reference|Terminals-and-concurrency]]
- [[Effects and realization reference|Effects-and-realization]]
- [[Packs, hosts and bodies reference|Packs-hosts-and-bodies]]
- [[Canon governance|Canon-governance]]

Conduit is experimental software. The development tree is substantial, but emulator proof, browser proof, physical hardware proof, human enactment, and released-product proof are intentionally different claims.
