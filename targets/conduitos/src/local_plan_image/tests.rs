use super::*;
use alloc::vec;
use conduit_core::*;
fn fixture() -> (Plan, Vec<u8>, &'static [u8]) {
    let source = b"type Example = U8\n";
    let identity = PlotIdentity {
        source_document_id: conduit_plot::syntax_source_document_identity(
            core::str::from_utf8(source).unwrap(),
        ),
        checked_plot_id: "fixture/checked".into(),
        expanded_plot_id: "fixture/expanded".into(),
    };
    let fragment = PlanFragment {
        plan_id: "unsealed".into(),
        fragment_id: "fixture/fragment".into(),
        source_document_id: identity.source_document_id.clone(),
        checked_plot_id: identity.checked_plot_id.clone(),
        expanded_plot_id: identity.expanded_plot_id.clone(),
        completion_policy: PlanCompletionPolicy::Live,
        realization_backs: vec![],
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: OfferGeneration(1),
        placements: vec![],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: vec![],
        connections: vec![],
        fore_ports: vec![],
        shared_pools: vec![],
        startup_dependencies: vec![],
        startup_order: vec![],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![],
        expected_sign: vec![],
        sign_storage_budget: SignStorageBudget {
            item_capacity: 1,
            byte_capacity: 128,
        },
        plan_fragments: vec![],
    };
    let plan = seal_plan(identity, vec![fragment]);
    let bytes = serde_json::to_vec(&plan).unwrap();
    (plan, bytes, source)
}
fn bounds() -> LocalPlanImageBounds {
    LocalPlanImageBounds {
        image_bytes: 65536,
        source_bytes: 1024,
        placements: 4,
        cords: 4,
    }
}
#[test]
fn exact_image_retains_reference_and_refuses_foreign_boot() {
    let (plan, bytes, source) = fixture();
    let decoded = DecodedLocalPlanImage::decode(&bytes, source, bounds()).unwrap();
    assert_eq!(decoded.reference_plan(), &plan);
    assert_eq!(decoded.encoded_extents(), (bytes.len(), source.len()));
    assert_eq!(
        decoded.plan_for_boot(&"fixture/host".into(), &"fixture/boot".into()),
        Ok(&plan)
    );
    assert_eq!(
        decoded.plan_for_boot(&"fixture/host".into(), &"new/boot".into()),
        Err(LocalPlanImageRefusal::Boot)
    );
}
#[test]
fn malformed_noncanonical_oversized_and_drifted_images_refuse() {
    let (mut plan, bytes, source) = fixture();
    let refusal = |image: &[u8], source: &[u8], bounds| {
        DecodedLocalPlanImage::decode(image, source, bounds)
            .err()
            .unwrap()
    };
    assert_eq!(
        refusal(&bytes, b"type Other = U8\n", bounds()),
        LocalPlanImageRefusal::Source
    );
    assert_eq!(
        refusal(b"{}", source, bounds()),
        LocalPlanImageRefusal::Encoding
    );
    let mut whitespace = bytes.clone();
    whitespace.push(b'\n');
    assert_eq!(
        refusal(&whitespace, source, bounds()),
        LocalPlanImageRefusal::NonCanonicalImage
    );
    let mut unknown = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
    unknown["unknown"] = true.into();
    assert_eq!(
        refusal(&serde_json::to_vec(&unknown).unwrap(), source, bounds()),
        LocalPlanImageRefusal::NonCanonicalImage
    );
    assert_eq!(
        refusal(
            &bytes,
            source,
            LocalPlanImageBounds {
                image_bytes: bytes.len() - 1,
                ..bounds()
            }
        ),
        LocalPlanImageRefusal::Bounds
    );
    assert_eq!(
        refusal(
            &bytes,
            source,
            LocalPlanImageBounds {
                placements: 0,
                ..bounds()
            }
        ),
        LocalPlanImageRefusal::Bounds
    );
    plan.fragments[0].boot_id = "changed/boot".into();
    assert_eq!(
        refusal(&serde_json::to_vec(&plan).unwrap(), source, bounds()),
        LocalPlanImageRefusal::Seal
    );
}
