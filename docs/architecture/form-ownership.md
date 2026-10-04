# Form ownership audit

**Owner:** [#4428](https://github.com/dancxjo/conduit/issues/4428)

This audit classifies handwritten encoding surfaces by ownership. It is a
migration inventory, not a claim that every candidate has already moved.

1. **Generated:** a checked Conduitese `form` is authoritative and
   target bindings contain no independent mapping.
2. **Generic:** machinery interprets arbitrary checked Type or Form
   graphs without owning a domain mapping.
3. **External:** an explicitly named protocol, provider, device, firmware ABI,
   or transport owns the mapping and a handwritten adapter is appropriate.

There is no “small stable handwritten mapping” category.

## Current generated and generic surfaces

| Surface | Class | Evidence |
|---|---:|---|
| Data save/load Text terminals | 1 | `semantics/data/types.conduit` selects a compact `u8` Form; authored Type order supplies iota tags, while checked language law supplies bounded invalid-tag refusal. Generated Rust and ECMAScript consume those same facts. |
| Audio tone terminal, PCM, gate and modulation discriminants | 1 | `semantics/audio/types.conduit` selects their exact one-byte Forms; generated bindings own the iota mappings, compatibility identities, bounds and invalid-input refusal. Gate uses an explicit exhaustive order to preserve its established compatibility independently of Type order. |
| Native Rust Type/value bindings | 2 | `architecture/plot/src/rust_binding/**` lowers checked Types and Forms without domain tables. |
| Canonical structured Info | 2 | `architecture/core/src/structured_info/**` encodes the generic checked value graph, not a domain schema. |

## Conduit-owned candidates to migrate

These paths contain domain-specific tags, field order, widths, bounds, or
malformed-input rules. Each needs a focused review and either a native
Form migration or a more precise external-boundary justification.

| Domain | Candidate surfaces |
|---|---|
| AI | `hybrid_retrieval_codec.rs`, `model_signature.rs`, `probability_digest.rs`, `source_extraction_codec.rs` |
| Artificial life | `lenia_line_frame.rs`, `lenia_region_wire.rs`, `reaction_diffusion_boundary_codec.rs` |
| Audio | compound frame layout in `audio_info.rs`, plus `sound_info.rs` and `pcm_clip.rs` |
| Catalog | `button_attempt_codec.rs`, `navigation_codec.rs`, `navigation_goal_codec.rs`, `pattern_comparison_codec.rs`, `sequence_normalization_codec.rs`, `timed_interval_codec.rs` |
| Data | `data_reference.rs`, `measurement_profile_wire.rs`, `measurement_threshold_wire.rs`, `measurement_wire.rs`, `tensor_codec.rs` |
| Human | `image_text_codec.rs`, `input_chord.rs`, `key_event.rs`, `human_interaction/canonical.rs` |
| Network | `network_info.rs`, `record_delivery.rs`, `record_transcript.rs`, `typed_record.rs` |
| Presentation | `bitmap.rs`, `graphics.rs` and nested graphics command codecs |
| Time | `historical_timeline_codec.rs`, `replay_codec.rs`, `retention_gap_codec.rs` |

The migration order is driven by the completed #4382 audit and reviewed #4375
follow-up slices. A migration must move
the authoritative table or layout, generate or generically interpret the
codec, delete the Rust duplicate, and retain exact compatibility proof.

## Named external boundaries

| Boundary | Class 3 reason |
|---|---|
| `architecture/wire/**`, `architecture/protected-line/**` | Conduit transport and protected-line sessions are separately versioned mechanism contracts. Their semantic payload Types remain native. |
| Body rendezvous CBOR/COSE | CBOR and COSE framing/interoperability are explicit protocol boundaries. |
| `mechanisms/protocols/midi/**`, Bluetooth framing | MIDI and Bluetooth specifications own their wire values. |
| Device packages under `mechanisms/devices/**` | External specifications own registers and frames. This classification does not assign their implementation permanently to Rust: [#4833 migrates ordinary protocols to checked plots](device-protocol-plots.md), retaining physical primitives and mandatory safety below them. |
| `semantics/web/src/http/codec.rs` | HTTP syntax is externally standardized; native request/response meaning remains separate. |
| Provider adapters under `targets/std/src/hosted_*` | Provider HTTP/JSON schemas are owned by those external services. |
| Browser, firmware, and distributed ABI modules under `targets/**` | Exact target or cross-process ABI contracts remain adapters; domain mappings inside them must still refer to native Types or Forms. |

External classification is not hereditary. A file at one of these boundaries
must not hide a Conduit-owned domain enum table merely because it also handles
an external frame.
