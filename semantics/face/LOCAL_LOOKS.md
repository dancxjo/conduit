# Personal Look proposals

`local-look.conduit` declares one personal visual proposal as ordinary Conduitese
startup data. The [example](examples/local-readable.conduit) changes body type
size and explicitly removes panel spacing. The other expression values inherit
from the author. A proposal contains at most 29 closed, typed fields; duplicate
fields refuse. There is no selector language, inheritance tree, specificity,
last-value rule, topology, action, binding or equipment selection.

The caller independently binds the personal resource; its identity is data, not
proof of a user or permission. `resolve_local_visual_look` retains the original
author and local resources, composes the supplied fields, and uses the existing
visual accessibility and Host data-admission path. It reports the selected
resource, fallback, accessibility overrides and each known field's origin.
A derived Look identity pins the exact author and local input bytes. An empty
proposal preserves the author's Look identity. Composing visual values preserves
the author's spoken and generative facets; a selected visual fallback cannot
select those facets for another Mask. A nonempty local visual proposal requires
an author visual facet; it cannot manufacture that facet. An empty proposal retains the ordinary missing-visual
fallback, with the original author and refusal still inspectable.

The independently supplied accessibility floor outranks a weaker personal
proposal; a personal proposal may request a stronger floor.
Standard restraints require nominal text token contrast of 4.5:1; enhanced
restraints require 7:1. Focus and meaningful structure tokens require 3:1. These
are conservative normal-text and non-text thresholds from WCAG 2.2
[1.4.3](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html),
[1.4.6](https://www.w3.org/WAI/WCAG22/Understanding/contrast-enhanced.html) and
[1.4.11](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html).
No large-text exception is inferred from an authored size alone. The check covers
primary/secondary text, emphasis, warning, failure and success against the five
named background surfaces, and focus/primary structure against those surfaces.
Decorative secondary structure and muted paint are not declared text tokens by
this check. Make prepares a finite sRGB table; runtime uses conservative integer
bounds and unrounded threshold comparisons. A failure names both fields and
their originating layers.

Forced platform colors take precedence. The result then says
`PlatformColorsRequired`, rather than pretending the proposed RGB tokens passed.
The selected Host must qualify its actual system colors, font resources, pixel
format, target sizes, focus, clipping and input behavior before acknowledging a
Show. Palette ratios alone do not establish WCAG conformance, rendering, a Back,
or a usable Show. The contrast requirement also applies to a selected fallback.

The existing Look version 2, Face grammar and bounds are unchanged. Both input
resources use the existing Look byte bound. Resolution accepts one author and
one local proposal, with fixed bounded requested/selected records and at most
29 supplied fields. The configured browser owner Mask consumes an independently
supplied local resource through this resolver, leaving its planned author Look
and complete Face unchanged. Its output carries the selected canonical Look and
a bounded local-resource digest, overridden fields and contrast outcome.

The browser SDK accepts canonical `LocalLookInfo` bytes with
`prepareOwnerFaceMask({ localLook: bytes, enhancedContrast: true })`.
These preferences remain local to that participation across Show refreshes;
`localLook: null` clears the personal proposal. They are not sent to the owner.
The independent enhanced-contrast floor cannot be weakened by the proposal.
Malformed, repeated, insufficient-contrast or unsupported local requests refuse
preparation before a usable Show; an unconfigured Mask cannot silently ignore
them. Omitting these options retains existing behavior.

This is browser preparation integration, not a claim of live DOM/owner action
delivery or WCAG conformance. Appearance UI, durable preference storage and the
native consumer remain separate work; the current Threebee preferences are
untouched.

Concrete startup defaults and explicit arguments must satisfy the same retained
Source laws, including bounds below a sequence/variant path. The tests keep the
regression found while authoring this resource. Native binding admission is also
required; it does not excuse a checker accepting a range-invalid default.

## Native prepared Masks

`NativeFaceMask::set_local_visual_look` independently binds typed personal data
and a contrast floor before presentation of a configured Mask. It uses the same
resolver as the browser preparation path; the original author configuration,
Plan and complete Face remain unchanged. `local_visual_look_resolution` exposes
the prepared resolution and field origins. It does not prove an available Show.
`NativeLocalLookRefusal` distinguishes native-value, resource-bound,
missing-visual-facet, duplicate-field, visual-admission and contrast failures.
An unconfigured Mask explicitly refuses personal preferences. The current native
scene contract cannot express selected control/panel corner radii, so personal
overrides of either field refuse with the field name, rather than pretending
those overrides rendered. Shared graphics evolution remains separate work.

A Look-only change over the same immutable Face uses the existing fresh-Mask
replacement: prepare a replacement, bind its personal data, transfer the local
encounter with `preserve_encounter_from`, suspend the old Mask, then present
and acknowledge the new Show. This preserves logical focus and refuses old Show
interactions. Republishing the same old surface revision remains refused;
changing style must not invent a new semantic Face revision.

The configured and capacity-fallback native scenes consume the same admitted
selection. Invalid local resolution cannot draw or acknowledge a usable Show.
CPU pixel writes and matched canonical native/browser selections are software
evidence; physical output, complete accessibility and Host deployment require
their own qualification. Appearance UI and durable preferences are not added.

Browser preparation requires a full owner snapshot for nondefault user or
platform constraints and for clearing a previously prepared nondefault context.
The SDK snapshots text-scale, forced-colors and reduced-motion preferences before
awaiting the owner response. An unchanged response cannot acknowledge applying
or clearing those preferences; the ABI preserves the prior available Show and
requests a full snapshot instead. These local preferences stay off the owner Line.
