# Conduit handbook

The website build renders this handbook at `/conduit/handbook/` with guided
navigation, canonical syntax highlighting, and the generated Plot diagrams.
This repository-owned Markdown remains the single authored source and the
GitHub Wiki remains its lightweight mirror.

**One continuing computer, made from the computers you have.**

Conduit separates portable meaning from the machinery that realizes it.
A **plot** says what should happen, a **plan** selects one exact realization,
and a **play** executes that plan. A **body** is the logical computer that can
continue as its hosts and connections change.

## Learn

Read these in order, or stop when you have enough context to try something:

1. [[Why Conduit|Why-Conduit]]: the purpose and the distinctions that make it work
2. [[Architecture tour|Architecture-tour]]: follow one plot into a plan and play
3. [[Conduitese]]: the language's basic shapes
4. [[Conduitese by example|Conduitese-by-example]]: read real source, then compose it

Browse [[Plot diagrams|Plot-diagrams]] to see checked plots as connected gears.

Keep the [[glossary|Glossary]] nearby; the terms are grouped by what they do.

## Use

- [[Start here|Start-here]]: open the product or run your first hosted plot
- [Try plots](https://github.com/dancxjo/conduit/blob/dev/docs/try-plots.md): check and run the reviewed examples
- [[Bodies, hosts, plans and plays|Bodies-hosts-plans-and-plays]]: understand lifecycle and realization
- [[State, time and data|State-time-and-data]]: choose the right temporal promise
- [[Lines, networking and replanning|Lines-networking-and-replanning]]: understand distributed work
- [[Face, mask and show|Face-Mask-and-Show]]: separate human meaning from rendering
- [[ConduitOS and real machines|ConduitOS-and-real-machines]]: find the target and hardware routes

## Reference

- [[Current language surface|Current-language-surface]]: supported spellings and executable evidence
- [[Architecture]]: vocabulary, semantic paths, and the callable fore
- [[Plots and flow|Plots-and-flow]]: composition, expressions, and completion
- [[Plot diagrams|Plot-diagrams]]: generated gear, port, and cord views
- [[Types and state|Types-and-state]]: values, bounds, retention, and data
- [[Terminals and concurrency|Terminals-and-concurrency]]: close, failure, cancellation, and pressure
- [[Effects and realization|Effects-and-realization]]: effects and exact realization
- [[Packs, hosts and bodies|Packs-hosts-and-bodies]]: source composition and construction
- [Architecture contracts](https://github.com/dancxjo/conduit/blob/dev/docs/architecture/README.md): implementation-facing references by topic

## Status and maintenance

[[Evidence and proof|Evidence-and-proof]] explains what each proof establishes.
[[Project status and roadmap|Project-status-and-roadmap]] points to the current
capability summary and open work. Development, accepted-release, and published
product truth are separate; check the
[current product surface](https://dancxjo.github.io/conduit/current-product.html)
for exact identities.

These pages teach the implemented development vocabulary: executable `plot`
and portable representation `form`. The paired migration in
[#4800](https://github.com/dancxjo/conduit/pull/4800) landed on `dev`; former
executable `form` and representation `code` declarations are rejected.
Published products may still lag that development surface.

The wiki is published from
[`wiki/` on `dev`](https://github.com/dancxjo/conduit/tree/dev/wiki).
Contribute edits there through a pull request; direct wiki edits are overwritten
by the repository's publication workflow. [[Canon governance|Canon-governance]]
retains design authority and provenance without making old issues the reading order.
