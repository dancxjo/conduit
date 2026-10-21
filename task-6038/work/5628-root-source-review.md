# Selected typography source review

Reviewed the uncompiled selected-typography-look worktree on 2026-10-21, based on 58979c509. This is source inspection, not validation or guest acceptance.

- Required closed VisualTypography intent has Editorial and Engineering alternatives, independent of density and palette. VisualExpressionInfo and Look use a coordinated v2 boundary; component recipes remain their existing version.
- Native private Typography maps the same five portable purposes to six immutable resources, with a separate Sans Title for Engineering. Both scene layout and compositor rendering consume selected profiles.
- Browser decoding checks the explicit intent; scoped title stacks consume a dedicated dataset field. Browser font appearance remains Host-specific.
- Owner replacement now refuses provisional/submitted input before requesting a replacement, including an erased but uncommitted draft. Named focus restoration precedes acknowledgement of the new Show.
- Review requested preference-state handling on failed scale/forced-color replacement and authoritative submitted-state completion. The source owner is addressing these before compiled proof.

Still required: checked-type fixture regeneration, focused contract/native/browser tests, resource ledger, actual same-Face two-Look owner-backed QEMU proof, focus/scale/forced-color checks, combined CI and appropriate stable evidence. No claim that the visual reference or general layout work is complete.
