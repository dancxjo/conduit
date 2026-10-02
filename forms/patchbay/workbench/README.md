# Patchbay

Patchbay lets you work with forms and inspect their realized execution. Open it
from a checkout with:

```sh
cargo xtask prove journey patchbay
cargo xtask prove journey patchbay --on browser
```

`host-contract` is the narrow Host-neutral boundary used by concrete workbench
realizations. It names the requested profile and returns exact bounded Play,
receipt and output evidence. The contract owns neither a Host implementation
nor planning policy; hosted targets implement it without depending upward on a
legacy Patchbay product model.

`svg-mask` is the lightweight static workbench Mask. It projects exact checked
Form Gears, named directional Ports, typed Cords, fan-in/fan-out junctions, and
Form boundary bindings into SVG or Mermaid without introducing another graph,
planner, or runtime. The `conduit diagram` entrance and documentation consumers
use this resident owner rather than carrying private layout implementations.

The installed product entrance is now simply `conduit`; Patchbay is not a
separate product command. See the
[current graphical ConduitOS journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)
for native Patchbay in use, or the [visual evidence guide](../../docs/visual-evidence.md)
for browser captures and provenance.

Patchbay is a resident inspect/edit/debug Form over authoritative form, plan,
play, body, host, boot, sign, and Observatory truth. Its graphical workbench is
a Mask realization of the Body's Face. Patchbay is not a separate application
universe, host, planner, runtime, capability registry, or source of current
realization truth.

Current ownership follows that boundary directly:

- `forms/patchbay/` owns the bounded resident control contract;
- `forms/patchbay/graph/` owns host-neutral semantic graph projection;
- `forms/patchbay/workbench/model/` composes resident workbench projections;
- `targets/browser/patchbay-workbench/` realizes the Mask in a browser;
- ConduitOS consumes the resident Form and its target-owned Mask directly.

Do not add universal Body/Form/Plan/Play/Host/Line/Sign truth to the workbench,
or preserve an ownership dependency through a compatibility re-export. Put semantic
truth with its ordinary owner and medium-specific geometry, interaction and
rendering with the workbench Mask.

Concrete hosted bootstrap and platform effects belong at the selected Host or
Mask realization edge. Patchbay projections consume exact advertisements,
plans, reports, and Observatory truth without depending on a concrete Host or
reconstructing a second current-truth registry.

## Explanation and proof classification

- Voyager scar explanations now live with the planner proof evidence they
  explain. Historically named presenter-plan descriptions remain reusable
  inspection projections: they consume exact supplied evidence and are used by
  ordinary catalog or inspection surfaces. Their legacy names are migration
  debt, not current architecture; they do not construct Hosts.
- The heterogeneous capstone baseline lives with its planner proof and is
  compiled only with that proof's tests.
- Planned renderer execution and Manifestation lifecycle now belong to
  universal Presentation truth. The legacy model retains only temporary
  adapter-offer preparation while native and HTML shells are retired.
- Retained capstone evidence is historical or proof-owned; it is not a product
  entrance and makes no physical claim unless its owning proof records one.
