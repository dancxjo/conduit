use super::*;
use crate::{
    DistributedRouteDemo, FormEditor, PatchbayModel, PatchbayRequestId, PatchbayTopology,
    PlanDocument, PlayDocument,
};
use conduit_std_host::{StdHost, ThreadTimer};

fn living_projection() -> (PatchbayPresentation, conduit_core::Plan) {
    let mut editor = FormEditor::from_source(
        "hello.conduit".into(),
        include_str!("../../../examples/hello.conduit").into(),
    )
    .unwrap();
    let selected = editor.view().checked.forms[0].items[0].identity.clone();
    assert!(editor.select_graph_item(&selected));
    let expanded = editor.expand_form("hello").unwrap();
    let mut host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let plan_document =
        PlanDocument::from_plan(PatchbayRequestId::new("renderer/plan").unwrap(), &plan).unwrap();
    let mut output = Vec::with_capacity(4096);
    let run = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut ThreadTimer)
        .unwrap();
    let play_document = PlayDocument::from_report(&plan, &run).unwrap();

    let patchbay = PatchbayModel::with_identity("renderer/host".into(), "renderer/boot".into());
    let mut topology = PatchbayTopology::new(1).unwrap();
    topology.ingest(&patchbay.startup_snapshot()).unwrap();
    let route = DistributedRouteDemo::build()
        .unwrap()
        .presentation()
        .clone();
    let presentation = PatchbayPresentation::new(
        7,
        editor.view(),
        Some(plan_document),
        Some(play_document),
        topology.current_report().cloned(),
        vec![route],
    )
    .unwrap();
    (presentation, plan)
}

#[test]
fn one_projection_preserves_exact_document_plan_play_route_and_evidence_facts() {
    let (presentation, plan) = living_projection();
    let identities = presentation.identities();
    assert_eq!(
        identities.source_document_id.as_ref(),
        Some(&plan.source_document_id)
    );
    assert_eq!(
        identities.document_checked_form_id.as_ref(),
        Some(&plan.checked_form_id)
    );
    assert_eq!(
        identities.plan_checked_form_id.as_ref(),
        Some(&plan.checked_form_id)
    );
    assert_eq!(identities.plan_id.as_ref(), Some(&plan.plan_id));
    assert_eq!(
        identities.expanded_form_id.as_ref(),
        Some(&plan.expanded_form_id)
    );
    assert_eq!(
        identities.active_play_id.as_ref(),
        presentation.play.as_ref().map(|play| &play.active_play_id)
    );
    for observation in &presentation.play.as_ref().unwrap().evidence {
        assert!(identities.evidence_ids.contains(&observation.evidence_id));
    }
    let route = &presentation.routes[0];
    assert_eq!(
        route.new_plan.prior.candidates[0].provider,
        conduit_core::ConnectionProvider::UsbCdc
    );
    assert!(presentation.selection().is_some());
}

#[test]
fn renderer_layout_and_disclosure_cannot_change_conduit_identities() {
    #[derive(Clone)]
    struct LocalRendererState {
        pan: (i32, i32),
        zoom_percent: u16,
        inspector_open: bool,
        node_order: Vec<String>,
    }

    let (presentation, _) = living_projection();
    let before = presentation.identities();
    let mut local = LocalRendererState {
        pan: (0, 0),
        zoom_percent: 100,
        inspector_open: true,
        node_order: vec!["first".into(), "second".into()],
    };
    local.pan = (900, -400);
    local.zoom_percent = 35;
    local.inspector_open = false;
    local.node_order.reverse();
    assert_eq!(presentation.identities(), before);
    assert_eq!(local.node_order, ["second", "first"]);
}

#[test]
fn projection_rejects_each_unbounded_collection() {
    let (presentation, _) = living_projection();

    let mut graph = presentation.document.clone();
    let item = graph.checked.forms[0].items[0].clone();
    graph.checked.forms[0].items = vec![item; MAX_RENDERER_GRAPH_ITEMS + 1];
    assert_eq!(
        PatchbayPresentation::new(8, graph, None, None, None, Vec::new()),
        Err(RendererProjectionError::GraphTooLarge)
    );

    let mut diagnostics = presentation.document.clone();
    let diagnostic = crate::EditorDiagnostic {
        code: "renderer-test",
        message: "bounded diagnostic".into(),
        span: diagnostics.checked.forms[0].items[0].source_span,
    };
    diagnostics.checked.diagnostics = vec![diagnostic; MAX_RENDERER_DIAGNOSTICS + 1];
    assert_eq!(
        PatchbayPresentation::new(8, diagnostics, None, None, None, Vec::new()),
        Err(RendererProjectionError::TooManyDiagnostics)
    );

    let route = presentation.routes[0].clone();
    assert_eq!(
        PatchbayPresentation::new(
            8,
            presentation.document.clone(),
            None,
            None,
            None,
            vec![route; MAX_RENDERER_ROUTES + 1],
        ),
        Err(RendererProjectionError::TooManyRoutes)
    );

    let mut route_candidates = presentation.routes.clone();
    let candidate = route_candidates[0].same_plan.plan.candidates[0].clone();
    route_candidates[0].same_plan.plan.candidates =
        vec![candidate; MAX_RENDERER_ROUTE_CANDIDATES + 1];
    assert_eq!(
        PatchbayPresentation::new(
            8,
            presentation.document.clone(),
            None,
            None,
            None,
            route_candidates,
        ),
        Err(RendererProjectionError::TooManyRouteCandidates)
    );

    let mut play = presentation.play.clone().unwrap();
    let observation = play.evidence[0].clone();
    play.evidence = vec![observation; MAX_RENDERER_EVIDENCE + 1];
    assert_eq!(
        PatchbayPresentation::new(
            8,
            presentation.document.clone(),
            presentation.plan.clone(),
            Some(play),
            None,
            Vec::new(),
        ),
        Err(RendererProjectionError::TooMuchEvidence)
    );

    let mut topology = presentation.topology.clone().unwrap();
    let host = topology.hosts[0].clone();
    topology.hosts = vec![host; MAX_RENDERER_TOPOLOGY_ITEMS + 1];
    assert_eq!(
        PatchbayPresentation::new(
            8,
            presentation.document.clone(),
            None,
            None,
            Some(topology),
            Vec::new(),
        ),
        Err(RendererProjectionError::TopologyTooLarge)
    );

    let mut plan = presentation.plan.clone().unwrap();
    plan.lines = vec![String::new(); MAX_RENDERER_INSPECTION_LINES + 1];
    assert_eq!(
        PatchbayPresentation::new(
            8,
            presentation.document.clone(),
            Some(plan),
            presentation.play.clone(),
            None,
            Vec::new(),
        ),
        Err(RendererProjectionError::InspectionTooLarge)
    );

    let mut oversized_plan = presentation.plan.clone().unwrap();
    let configuration = oversized_plan.exact.fragments[0].placements[0].configuration[0].clone();
    oversized_plan.exact.fragments[0].placements[0].configuration =
        vec![configuration; MAX_RENDERER_PLAN_ITEMS + 1];
    assert_eq!(
        PatchbayPresentation::new(
            8,
            presentation.document.clone(),
            Some(oversized_plan),
            None,
            None,
            Vec::new(),
        ),
        Err(RendererProjectionError::PlanTooLarge)
    );

    let mut source = presentation.document.clone();
    source.source = "x".repeat(crate::MAX_FORM_SOURCE_BYTES + 1);
    assert_eq!(
        PatchbayPresentation::new(8, source, None, None, None, Vec::new()),
        Err(RendererProjectionError::SourceTooLarge)
    );
}

#[test]
fn projection_rejects_a_drifting_document_plan_or_play_identity() {
    let (presentation, _) = living_projection();
    let mut plan = presentation.plan.clone().unwrap();
    plan.checked_form_id = "checked/drift".into();
    assert_eq!(
        PatchbayPresentation::new(
            8,
            presentation.document.clone(),
            Some(plan),
            presentation.play.clone(),
            presentation.topology.clone(),
            presentation.routes.clone(),
        ),
        Err(RendererProjectionError::IdentityMismatch)
    );

    let mut play = presentation.play.clone().unwrap();
    play.plan_id = "plan/drift".into();
    assert_eq!(
        PatchbayPresentation::new(
            8,
            presentation.document,
            presentation.plan,
            Some(play),
            presentation.topology,
            presentation.routes,
        ),
        Err(RendererProjectionError::IdentityMismatch)
    );
}
