# Conduit product surfaces

Open the [Body Workspace](https://dancxjo.github.io/conduit/workspace/) to birth
or return to a Body. The same surface owns its Forms, lifecycle, Hosts,
tutorial, and inspection. See the [ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)
for the freestanding graphical realization.

| Surface | Role | Source guide |
| --- | --- | --- |
| CLI | Runs and inspects Forms, Bodies, and installed product experiences. | `conduit/` |
| Body Workspace | The primary product surface: zero-Body bootstrap, resident Forms, lifecycle, Hosts, tutorial, and inspection. | [Workspace](workspace/README.md) |
| Tour | A resident tutorial Form plus a retained standalone compatibility entrance. | [Tour](tour/README.md) |
| Crèche | A retained compatibility and target-preparation entrance over the Workspace-owned bootstrap. | [Crèche](creche/README.md) |
| Patchbay | A resident inspect/edit/debug Form plus specialized development and compatibility entrances. | [Patchbay](patchbay/README.md) |

These surfaces use the same checker, planner, kernel, and semantic Presentation
contracts. Their models describe application state; renderers bind local input
and display to that state. Compatibility routes do not own another Body,
scheduler, lifecycle, or bootstrap model. Reusable visual assets belong in `shared/`.

The installed command-line entrance lives in `conduit/`; target Hosts
live in [targets](../targets/README.md). See the [documentation index](../docs/README.md)
for architecture, setup, and the current work map.
