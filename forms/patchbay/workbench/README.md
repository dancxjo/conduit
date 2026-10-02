# Patchbay workbench specimens

This directory owns bounded authored specimens used to exercise the Patchbay
workbench Mask role. A specimen describes semantic parts, finite resources and
links together with workbench placement hints; it is not Host observation,
authority, a Plan, or a live environment.

The current native workbench consumes `examples/maker-workbench.json` for the
environment and PREWAKE journeys. Keeping that authored input with the resident
Patchbay Form family avoids making a particular native product shell its
semantic owner.

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
