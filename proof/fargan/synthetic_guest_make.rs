// Explicit numerical topology proof appliance, separate from product profiles.
// Make requires a nonempty compiled inventory; retain its existing text-literal
// owner. This proof executes selected numeric/Source owners, not that text owner.
pub const EMBEDDED_MAKE: MakeRecord = MakeRecord {
    schema: MAKE_SCHEMA,
    profile_id: "profile:conduitos-synthetic-numeric-topology-v1",
    build_id: "build:synthetic-numeric-local-proof",
    image_binding: "image:synthetic-numeric-local-proof",
    target: "conduitos/x86_64/pc",
    implementations: IMPL_TEXT_LITERAL,
    facilities: 0,
    resources: 0,
    bases: 0,
    drivers: 0,
    presenters: 0,
    proof_instrumentation: PROOF_NUMERIC_TOPOLOGY,
    presentation_surface_slots: 0,
    presentation_surface_bytes: 0,
    runtime_arena_ceiling: 268435456,
    operation_slot_ceiling: 1024,
    timer_slot_ceiling: 1,
    evidence_item_ceiling: 32768,
};
