# Conduit products

Start with the [Tour](https://dancxjo.github.io/conduit/tour/) to learn by
running Forms, or see the [ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)
for the freestanding graphical experience.

| Product | What it does | Source guide |
| --- | --- | --- |
| CLI | Runs and inspects Forms, Bodies, and installed product experiences. | `conduit/` |
| Tour | Teaches Forms, Hosts, and Bodies through executable lessons. | [Tour](tour/README.md) |
| Crèche | Births a Body, selects initial Forms, and prepares its first Hosts. | [Crèche](creche/README.md) |
| Patchbay | Authors and inspects Forms and projects current execution and Body state. | [Patchbay](patchbay/README.md) |

These products use the same checker, planner, kernel, and semantic Presentation
contracts. Their models describe application state; renderers bind local input
and display to that state. Reusable visual assets belong in `shared/`.

The installed command-line entrance lives in `conduit/`; target Hosts
live in [targets](../targets/README.md). See the [documentation index](../docs/README.md)
for architecture, setup, and the current work map.
