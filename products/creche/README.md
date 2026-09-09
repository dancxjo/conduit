# Crèche

Crèche helps you birth a Body, choose zero or more initial Forms, prepare its
first Host, and add machinery before graduating to ordinary operation.
[Open Crèche](https://dancxjo.github.io/conduit/creche/), or reach it from the
[Tour](../tour/README.md).

- `model/` owns portable Crèche presentation state.
- `browser/` owns the browser application and its target-selection, spore,
  download, and graduation interactions.
- `../../targets/browser/host/` supplies browser hosting; target-specific
  fabrication and deployment remain with their target families.

Crèche consumes the reviewed [Form inventory](../../forms/README.md) and births
workload revision zero. It uses the ordinary Body lifecycle rather than owning
a scheduler or a permanent Body work surface. Artifact construction, flashing,
observed Boot, and admission as a Part remain separate steps: downloading a
spore does not prove that its target booted or joined.

The current development birth surface is one shared application: `model/`
produces its semantic view and accepts revision-bound naming, search, selection,
and Birth actions. Native ConduitOS renders that view with its keyboard controls;
the browser transports the same view and actions through its runtime. Host
adapters supply the checked Form inventory and perform authoritative lifecycle
review after the shared draft requests Birth.

The opening interaction is name → Forms → Birth. Browser Host provisioning and
source/evidence inspection remain available as secondary details. The native
arrival currently offers the Keyboard canvas Form and continues into its Wake
and Play; browser birth retains its existing first-Host provisioning workflow.
A successful draft action alone does not create a Body, admit a Host, or start
execution.
