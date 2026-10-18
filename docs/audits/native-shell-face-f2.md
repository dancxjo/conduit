# Native shell Face action binding

The native shell retains its existing application surfaces and local navigation. Lifecycle keyboard shortcuts use the current owner Face through an actual rendered and acknowledged NativeFaceMask Show before owner admission. A shortcut captured before that Show is rendered is not evidence that a human selected an action from the newly shown surface.

| Surface or content | Previous behavior | Corrected behavior |
| --- | --- | --- |
| Ordinary Home or Body lifecycle shortcut | Resolve semantic action and call the lifecycle owner directly | Render and acknowledge the current Face with NativeFaceMask, submit its exact action, validate current owner and Show, then restore the application surface |
| Explicit Face inspection | NativeFaceMask submits a Show-bound interaction | Retained |
| Crèche birth | FaceArrival admits birth through its own acknowledged Mask Show | Retained |
| Home launcher, Plots, and Prompt | Scene contains labels and command results absent from the owner Face | Face also carries current destination labels, installed titles, selection, command and result context |
| Selected resident Plot | Face uses entrance name and inspection-only wording despite resident IDs | Face names the exact foreground resident and describes its Body-workset provenance |
| Zero-body entrance | Embedded checked Plot and inspection-only context | Retained |

Home selection, command editing, view changes and inspection remain local reading state. Home's `wake` command still reports unavailable authority. Home Tour, Patchbay and Plot actions select existing residents; the current `RunPlot` branch selects a resident and does not itself admit Plan or Play. These context disclosures introduce no Home lifecycle actions or new authority.

Focused shell audit coverage checks current Home content, unavailable command results, unchanged lifecycle actions, exact resident identity and name, and retained zero-body entrance wording. Lifecycle admission tests separately cover stale owner/Face/Show and refusal paths. Local and emulator evidence do not establish physical-device or human acceptance.
