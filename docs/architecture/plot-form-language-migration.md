# Plot and Form language migration audit

Issue [#4513](https://github.com/dancxjo/conduit/issues/4513) establishes the
paired canonical vocabulary:

```text
type -> form
plot -> plan -> play
```

A Type owns meaning and a Form owns one concrete portable representation of
that meaning. A Plot owns portable authored executable meaning, a Plan is one
admitted realization for particular bodies and hosts, and a Play is that Plan
in execution. The grammar rejects executable `form`, representation `code`,
and `representation` as compatibility aliases.

## Identity and schema decisions

- The existing compact-Form compatibility digest retains its historical
  `conduit.code.u8@1` salt. Renaming the concept therefore does not change
  encoded bytes, discriminants, or compatibility identities.
- Current generated inventory/report schemas move to Plot-named, incremented
  versions: reviewed Plot inventory v2, Plot conformance report v6, Crèche
  reviewed Plot inventory v2, Crèche bundle v2, workspace reviewed Plot
  catalog v3, and Tour gallery v2.
- Frozen `CND-FRM-*` diagnostic identifiers remain stable identifiers rather
  than current user-facing architectural vocabulary.

## Remaining `form` classification

The case-insensitive repository audit classifies survivors as follows:

1. Canonical Type-Form vocabulary: the `form` keyword, `TypeFormSyntax`,
   `CheckedTypeForm`, generated `*Form` codecs, and their fields and tests.
2. Unrelated ordinary or domain language: HTML/UI form fields, waveforms,
   transforms, conformance, formatting, and ordinary phrases such as “in the
   form of”.
3. Frozen compatibility or history: the digest salt and diagnostic identifiers
   above, archived documents/evidence, and v1-history fixtures.
4. Third-party/platform terminology: HTML forms, `FormData`, and vendored or
   licensed text.

No survivor denotes the former executable Form concept.

## Scientific Plot collision

`MeasurementPlot` and `presentation/measurement-plot` remain the scientific
visualization concept. The one compound where a blind migration would repeat
the word instead uses the role-bearing `measurement_plot_source_*` spelling.
No doubled Plot identifier is admitted.
