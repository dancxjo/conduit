# Crèche compatibility entrance

This retained package is a compatibility entrance. Opening Crèche redirects to
the Body Workspace, which owns zero-Body bootstrap, reviewed initial workset,
birth, and the transition to ordinary Body operation.

- The body Workspace owns zero-body bootstrap and reviewed initial-workset state.
- `model/` remains for compatibility and target-preparation presentation state.
- `browser/creche.mjs` is redirect-only and owns no lifecycle or durable state.
- reusable target-selection, spore, and release modules remain packaged for
  Workspace Host fabrication; they do not constitute a Crèche application.
- `../../targets/browser/host/` supplies browser hosting; target-specific
  fabrication and deployment remain with their target families.

Workspace consumes the reviewed [form inventory](../../forms/README.md) and
births workload revision zero. Artifact construction, flashing, observed boot,
and admission as a Part remain separate steps: downloading a spore does not
prove that its target booted or joined.

The current development birth surface is one shared application: `model/`
produces its semantic view and accepts revision-bound naming, search, selection,
and Birth actions. Native ConduitOS renders that view with its keyboard controls;
the browser transports the same view and actions through its runtime. host
adapters supply the checked form inventory and perform authoritative lifecycle
review after the shared draft requests Birth.

The opening interaction is name → Forms → Birth in Workspace. A successful
draft action alone does not create a Body, admit a Host, or start execution.
