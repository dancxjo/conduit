# Patchbay

Patchbay lets you work with plots and inspect their realized execution. Open it
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
Plot Gears, named directional Ports, typed Cords, fan-in/fan-out junctions, and
Plot boundary bindings into SVG or Mermaid without introducing another graph,
planner, or runtime. The `conduit diagram` entrance and documentation consumers
use this resident owner rather than carrying private layout implementations.

The installed product entrance is now simply `conduit`; Patchbay is not a
separate product command. See the
[current graphical ConduitOS journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)
for native Patchbay in use, or the [visual evidence guide](../../../docs/visual-evidence.md)
for browser captures and provenance.

Patchbay is a resident inspect/edit/debug Plot over authoritative plot, plan,
play, body, host, boot, sign, and Observatory truth. Its graphical workbench is
a Mask realization of the Body's Face. Patchbay is not a separate application
universe, host, planner, runtime, capability registry, or source of current
realization truth.

Current ownership follows that boundary directly:

- `plots/patchbay/` owns the bounded resident control contract;
- `plots/patchbay/graph/` owns host-neutral semantic graph projection;
- `plots/patchbay/workbench/model/` composes resident workbench projections;
- `targets/browser/patchbay-workbench/` realizes the Mask in a browser;
- ConduitOS consumes the resident Plot and its target-owned Mask directly.

Do not add universal Body/Plot/Plan/Play/Host/Line/Sign truth to the workbench,
or preserve an ownership dependency through a compatibility re-export. Put semantic
truth with its ordinary owner and medium-specific geometry, interaction and
rendering with the workbench Mask.

Concrete hosted bootstrap and platform effects belong at the selected Host or
Mask realization edge. Patchbay projections consume exact advertisements,
plans, reports, and Observatory truth without depending on a concrete Host or
reconstructing a second current-truth registry.

## Catalog authoring and workspace layouts

The development authoring surface uses the semantic catalog's exact Kind
revision, Fore, startup parameters, configuration rules, and finite limits.
Visibility in this catalog is distinct from authorability in the selected
source profile, and neither promises a current Host Back. Cord highlighting
queries the checked editor; configuration uses the source checker's validator.
Reviewed adapter suggestions name an explicit Gear and its exact ports. Placing
one does not silently connect it. Generated SVG descriptions expose the same
canonical port and configuration facts for reference and handbook consumers.

`patchbay_application::PatchbayWorkspace` is the portable presentation document,
separate from authored source, checked Plot, and live realization. Its
`conduit.patchbay.workspace/v1` schema admits at most four named layouts in
64 KiB. Each layout has bounded semantic-subject positions, Cord routes,
visual frames, notes, collapsed subjects, viewport, and preferred lens.
Coordinates, annotation text, membership counts, and route points have explicit
limits. Frames create no semantic scope; route points create no Gear or Line.

Browser **Workspace layouts and annotations** controls save, switch, import,
and export these documents through admitted application storage and the same
portable validator. Export permits personal or shared arrangements without
requiring one global layout. A layout may remember a Plan/Play/Signs lens, but
all its evidence is projected again from the current authoritative snapshot.
No runtime evidence is serialized into the workspace.

Workspace correlation retains exact source and checked Plot identities.
Unknown subjects are reported as orphans, never matched by position or label.
A changed basis refuses application instead of guessing a rename mapping.
Legacy Flow v1 positions migrate deterministically only when their exact
projection supplies semantic subject mappings; unknown/future schemas refuse
without overwriting the retained document. Workspace migration never edits a
Plot, and semantic edits never fabricate replacement geometry.

The existing `cargo xtask prove patchbay-body-workbench` entrance includes
catalog, portable workspace, diagram-reference, and browser authoring proofs.
These are development contract/browser evidence, not a new stable-release,
physical-device, or human-enactment claim. Native consumers share the portable
schema and checking laws; identical graphical controls are not required.

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
