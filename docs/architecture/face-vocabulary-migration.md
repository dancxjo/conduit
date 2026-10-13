# Face vocabulary migration

Authored human-facing construction uses `face/*`. The semantic carrier crate
is `conduit-face` at `semantics/face`, and its universal carrier is `Face`.
The previous Body projection wrapper is `BodyFace`; it carries context, focus,
the universal `face`, and operator actions. Its generative request adapter is
`from_body_face`, while `from_face` accepts the universal carrier directly.

## Classified changes

- Gear namespaces, Rust crate/dependency names, module and source paths,
  `Presentation*` semantic types, snake-case API names, constant names,
  authored Type names, live manifests, live fixtures and documentation links
  adopt Face vocabulary together.
- Domain and representation vocabulary stays independent. In particular,
  `representation`, transport representation shapes, and external media
  presentation timestamps do not name the universal Face carrier.
- Retained evidence under checked-in evidence directories keeps its original
  bytes and historical spelling. Historical receipts are not regenerated or
  claimed to prove the migrated tree. Source that verifies historical receipts
  continues to recognize their recorded identities. `EvidenceProvenance` reads
  the historical `presentation_id` and `presentation_revision` receipt keys
  through deserialize-only aliases; new receipts write `face_id` and
  `face_revision`. This does not admit retired authored Gear names.
- Canonical semantic names and encoded field keys change with the public
  vocabulary. A source/Type/Plan/Face identity calculated from these names must
  be recalculated from the current tree; an old receipt is not an alias for it.
  No backwards alias of the retired authored `presentation/*` namespace is
  introduced.

The [construction-name contract](face-data-names.md) requires an explicit
bounded meaningful label for every authored `face/*` Gear, independently of
its other configuration. Local checks establish only the checked
working tree; integration, promotion, publication and human proof require their
own exact-source evidence.

## Palette construction

The Patchbay palette collects a data name before placing a Face Gear. The
existing configuration controls render the catalog's checked `DataLabel` rule.
The semantic `PlaceGear` request carries optional `data_label` Text; Face
construction requires it, and ordinary non-Face placement preserves its
existing defaults. `PlotEditor::place_palette_kind_with_label` validates the
name before changing source or revision. The unnamed entrance refuses Face
placement with a data-name diagnostic. No name is inferred from a Gear ID or
catalog title.
