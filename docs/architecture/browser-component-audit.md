# Browser component foundation audit (#5376)

This audit compares [#5376](https://github.com/dancxjo/conduit/issues/5376)
with the Thermostat foundation merged in
[#5374](https://github.com/dancxjo/conduit/pull/5374).
It describes development implementation and checked-in proof coverage, not a
fresh run of the installed-owner journey or accepted-release evidence.

| Requirement | Merged implementation | Remaining acceptance |
|---|---|---|
| Real reusable component | `targets/browser/handbook/owner-face-component.mjs` registers `conduit-owner-face`, renders an open Shadow DOM, and consumes generic Face subjects, values, actions, and exclusive choices. It contains no thermostat domain state. | The Handbook creates this class directly. This is not yet selection of a digest/version-bound certified Gear Back with checked Fore, slots, offers and resource caps. |
| Trusted actions and Body state | `owner-participation.mjs` passes the component's actions to `submitOwnerFaceInteraction`, checks the current Show, and refreshes the Owner Face. The SDK owns Mask preparation and acknowledgement. Thermostat state remains in the retained native Body Plot. | Do not introduce a second dispatcher. The exact same committed Todo Body must still be compared through this component and the generic renderer. |
| Generic fallback | The standard-element renderer remains at `targets/browser/host/assets/application-presentation.mjs`; unfamiliar choice contracts retain generic subject/action rendering inside the component. | The Handbook imports the module unconditionally. A missing module, unsupported component or failed component stylesheet does not yet have a proved automatic selected-Back fallback. |
| Swappable Looks | The component has one linked `owner-face-look.css` with responsive styling. | Responsive desktop/mobile screenshots are not two admitted Looks. Named parts were missing; this slice exposes them. Look selection, checked tokens and admitted CSS remain #5366/#5379 work. |
| Accessibility | Native buttons, labelled inputs/selects, fieldset/legend and labelled radios, plus focus-visible CSS, provide ordinary control semantics. | The thermostat driver covers named controls and keyboard/pointer activation, but does not prove full traversal, focus continuity across replacement Shows, or the full accessibility failure matrix. Value paragraphs have `aria-label`, but Chromium does not expose those paragraphs with that accessible name; semantic value naming remains a gap. |
| Live evidence | `proof/browser/thermostat-owner-browser.mjs` checks source-pinned installed Owner state, Face/Show identities, retained Body Plan/Play provenance, keyboard/pointer controls, stale-Show refusal, limits, screenshots and mobile overflow. | This is Thermostat evidence, not the required Todo comparison. Forged events, invalid/mismatched inputs, missing Host/document/module, capacity failures, CSS failure, and restricted implementation effects still need explicit component conformance coverage. Ordinary same-realm JavaScript registration is not hostile-code confinement. |

## Small completed slice: named styling parts

The existing class now exposes `document`, `subject`, `collection`, `value`,
`action`, `label`, `input`, `button`, `choice-group`, and `choice` parts.
These names describe rendering roles, not application identities. Existing
classes and the default stylesheet remain the internal rendering route.
Author CSS can address these parts through `conduit-owner-face::part(...)`.
No module loading, application dispatch, state store or Look admission route is
added by this contract.

`proof/browser/owner-face-component.spec.mjs` uses the pinned Chromium project,
one worker and zero retries. It switches compact/spacious CSS on one element,
checks actual computed padding and unchanged Shadow DOM/control identity,
then checks accessible button naming, focus, keyboard and pointer callback
routing, unavailable-Show disabling, and native semantics without styling.
It is included in the existing `cargo xtask prove browser-host` suite.
Its Face and callback are renderer fixtures: they prove the styling seam and
native interactions, not Body mutation, Show acknowledgement, certification,
or the same-Body Todo acceptance required to close #5376.
