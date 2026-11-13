//! Only descriptive names leave the resident execution projection.
use super::*;
#[test]
fn resident_projection_retains_full_plan_identity_and_original_evidence_budget() {
    let (d, p) = context();
    let image = prepare(&d, &p);
    let plan = plan(&d, &p, &image, 1000);
    let f = &plan.fragments[0];
    let binding = image
        .bind(&plan, &f.fragment_id, &f.placements[1].placement_id)
        .unwrap();
    let g = generated(&plan);
    let encoded =
        encode_assigned_resident_native_plan(&g, AssignedPlanMaxima::TINY_HOST, &[binding])
            .unwrap();
    // Resident projection removes only an unused descriptive catalog. The
    // complete sealed Plan and generated evidence budget remain authoritative.
    assert_eq!(g.signs.len(), f.expected_sign.len());
    assert!(!g.signs.is_empty());
    assert_eq!(g.sign_items, f.sign_storage_budget.item_capacity);
    assert_eq!(g.sign_bytes, f.sign_storage_budget.byte_capacity);
    assert!(AssignedPlanRecords::new(&encoded)
        .unwrap()
        .map(Result::unwrap)
        .all(|r| r.tag != ASSIGNED_SIGN));
    let literal = encode_assigned_native_plan(
        &g,
        AssignedPlanMaxima {
            encoded_bytes: u16::MAX,
            runtime_state_bytes: u16::MAX,
            counts: [u8::MAX; ASSIGNED_PLAN_COUNT_KINDS],
        },
    )
    .unwrap();
    assert_eq!(
        AssignedPlanRecords::new(&literal)
            .unwrap()
            .map(Result::unwrap)
            .filter(|r| r.tag == ASSIGNED_SIGN)
            .count(),
        g.signs.len()
    );
    assert_eq!(&literal[12..16], &encoded[12..16]);
    assert_eq!(&literal[16..80], &encoded[16..80]);
    // Generation did not rewrite any part of the semantic Plan.
    assert_eq!(g.signs.len(), f.expected_sign.len());
    assert_eq!(g.plan_id, plan.plan_id.as_str());
}
