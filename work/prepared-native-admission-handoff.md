# Prepared Native admission (UNBUILT)

Ownership: three manifest paths only. Base c8946a6f3. No builds, commits, Source edits, compiler changes, or root changes.

## Existing lawful path
Language analysis_epoch_initialization::admit explicitly constructs ParserState from every raw field, then ParserHypothesis and ScorerHypothesis; admit_beam repeats for all four slots then constructs Beam. The generated from_structured decoder checks exact semantic Type, recursively decodes every child, then invokes each constructor. Contracts are flattened; CheckedNativeType.invariants is local. Therefore checking only a Beam's root laws is insufficient: nested ParserState cycle/root/committed-edge laws must execute too.

## New seam
rust_binding::PreparedNativeAdmission::new(&CheckedSyntaxDocument, &KindId, PreparedNativeBounds) selects exact Native metadata from the checked document, including private retained imported metadata, rejecting conflicting aliases and missing metadata. It builds a finite schema occurrence/path closure and prepares each actual retained invariant using PreparedPortableExpressionEvaluator. Bounds: caller canonical byte bound within existing Core maximum; <=4096 schema visits/scopes/laws; path depth <=64. Collection/sequence maximum occurrences and every variant alternative are prepared; absent sequence items/inactive alternatives are skipped at admission. Primitive refinements use the existing borrowed contract traversal.

begin borrows/copies actual canonical bytes into admitted capacity and validates exact target shape/Type. step validates one schema scope: existing borrowed contracts and its actual checked Boolean law programs, using preallocated canonical scratch with the selected subvalue's own unchanged type bytes. No numeric/domain policy or Source construction legality is duplicated. All scopes must complete before admitted returns a private borrowed witness. Retained input, exact document/metadata and invariant output getters remain available after failure/cancel. Pressure before copying preserves old evidence; failures are terminal. retire_admitted explicitly permits reuse only after success, and Rust borrowing prevents retiring while a witness remains borrowed.

Tests prepare exact fields using existing Core PreparedStructuredComposer, never replace raw prefixes: wrong Raw schema refuses, target field composition passes shape but illegal start>end still fails actual Source invariant. Nested law/contract, foreign identity, pressure/cancel, finite-bound refusal and allocation-probe cases included, all UNRUN.

## Deliberate scope/remaining gates
This validates canonical Native values against supplied *checked-document metadata*, following the repository's checked-document preparation trust boundary; it does not attest Source authenticity against a separate source string/digest, instantiate generated Rust values, or grant model/current/commit authority. Checked documents' public Native metadata must remain under the checking owner's custody (as in current expansion/binding APIs).

Unknown structural schema identities and consecutive nominal wrappers are refused rather than silently dropping laws; the latter is a bounded initial profile because Core's borrowed representation() strips all nominal wrappers. Needs actual full Language Beam preparation check at integration: unsupported anonymous schemas would require an explicit lawful metadata/profile extension, not omission.

One step is one finite schema scope, not one expression opcode. Each evaluator already has its own prepared finite execution bounds. Total retained evaluator heap is finite but this API does not report an aggregate heap-byte formula. This is a generic admission prerequisite, not the full installed controller. No claim that allocation tests, compilation, actual Language law closure, or runtime/consumer integration have passed.

Suggested root first check: cargo +stable test -p conduit-plot --test prepared_native_admission; then prepare/admit actual Language Raw->refined Beam field-copy fixture before integrating with the retained round cursor. Root owns all builds and Source/full-controller wiring.
