use super::*;
use crate::{
    DistributedRouteDemo, PatchbayModel, PatchbayRequestId, PatchbayTopology, PlanDocument,
    PlayDocument, PlotEditor,
};
use conduit_std_host::{StdHost, ThreadTimer};

fn living_projection() -> (PatchbayFace, conduit_core::Plan) {
    let mut editor = PlotEditor::from_source(
        "hello.conduit".into(),
        include_str!("../../../../../plots/hello/main.conduit").into(),
    )
    .unwrap();
    let selected = editor.view().checked.plots[0].items[0].identity.clone();
    assert!(editor.select_graph_item(&selected));
    let expanded = editor.expand_plot("hello").unwrap();
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
    let route = DistributedRouteDemo::build().unwrap().face().clone();
    let face = PatchbayFace::new(
        7,
        editor.view(),
        Some(plan_document),
        Some(play_document),
        topology.current_report().cloned(),
        vec![route],
    )
    .unwrap();
    (face, plan)
}

#[test]
fn one_projection_preserves_exact_document_plan_play_route_and_sign_facts() {
    let (face, plan) = living_projection();
    let identities = face.identities();
    assert_eq!(
        identities.source_document_id.as_ref(),
        Some(&plan.source_document_id)
    );
    assert_eq!(
        identities.document_checked_plot_id.as_ref(),
        Some(&plan.checked_plot_id)
    );
    assert_eq!(
        identities.plan_checked_plot_id.as_ref(),
        Some(&plan.checked_plot_id)
    );
    assert_eq!(identities.plan_id.as_ref(), Some(&plan.plan_id));
    assert_eq!(
        identities.expanded_plot_id.as_ref(),
        Some(&plan.expanded_plot_id)
    );
    assert_eq!(
        identities.active_play_id.as_ref(),
        face.play.as_ref().map(|play| &play.active_play_id)
    );
    for observation in &face.play.as_ref().unwrap().signs {
        assert!(identities.sign_ids.contains(&observation.sign_id));
    }
    let route = &face.routes[0];
    assert_eq!(
        route.new_plan.prior.candidates[0].base,
        conduit_core::BaseImplementationId::from("conduit.base/usb-cdc-acm@1")
    );
    assert!(face.selection().is_some());
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

    let (face, _) = living_projection();
    let before = face.identities();
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
    assert_eq!(face.identities(), before);
    assert_eq!(local.node_order, ["second", "first"]);
}

#[test]
fn projection_rejects_each_unbounded_collection() {
    let (face, _) = living_projection();

    let mut graph = face.document.clone();
    let item = graph.checked.plots[0].items[0].clone();
    graph.checked.plots[0].items = vec![item; MAX_RENDERER_GRAPH_ITEMS + 1];
    assert_eq!(
        PatchbayFace::new(8, graph, None, None, None, Vec::new()),
        Err(RendererProjectionError::GraphTooLarge)
    );

    let mut diagnostics = face.document.clone();
    let diagnostic = crate::EditorDiagnostic {
        code: "renderer-test",
        message: "bounded diagnostic".into(),
        span: diagnostics.checked.plots[0].items[0].source_span,
    };
    diagnostics.checked.diagnostics = vec![diagnostic; MAX_RENDERER_DIAGNOSTICS + 1];
    assert_eq!(
        PatchbayFace::new(8, diagnostics, None, None, None, Vec::new()),
        Err(RendererProjectionError::TooManyDiagnostics)
    );

    let route = face.routes[0].clone();
    assert_eq!(
        PatchbayFace::new(
            8,
            face.document.clone(),
            None,
            None,
            None,
            vec![route; MAX_RENDERER_ROUTES + 1],
        ),
        Err(RendererProjectionError::TooManyRoutes)
    );

    let mut line_candidates = face.routes.clone();
    let candidate = line_candidates[0].same_plan.plan.candidates[0].clone();
    line_candidates[0].same_plan.plan.candidates =
        vec![candidate; MAX_RENDERER_ROUTE_CANDIDATES + 1];
    assert_eq!(
        PatchbayFace::new(8, face.document.clone(), None, None, None, line_candidates,),
        Err(RendererProjectionError::TooManyRouteCandidates)
    );

    let mut play = face.play.clone().unwrap();
    let observation = play.signs[0].clone();
    play.signs = vec![observation; MAX_RENDERER_SIGNS + 1];
    assert_eq!(
        PatchbayFace::new(
            8,
            face.document.clone(),
            face.plan.clone(),
            Some(play),
            None,
            Vec::new(),
        ),
        Err(RendererProjectionError::TooManySigns)
    );

    let mut topology = face.topology.clone().unwrap();
    let host = topology.hosts[0].clone();
    topology.hosts = vec![host; MAX_RENDERER_TOPOLOGY_ITEMS + 1];
    assert_eq!(
        PatchbayFace::new(
            8,
            face.document.clone(),
            None,
            None,
            Some(topology),
            Vec::new(),
        ),
        Err(RendererProjectionError::TopologyTooLarge)
    );

    let mut plan = face.plan.clone().unwrap();
    plan.lines = vec![String::new(); MAX_RENDERER_INSPECTION_LINES + 1];
    assert_eq!(
        PatchbayFace::new(
            8,
            face.document.clone(),
            Some(plan),
            face.play.clone(),
            None,
            Vec::new(),
        ),
        Err(RendererProjectionError::InspectionTooLarge)
    );

    let mut oversized_plan = face.plan.clone().unwrap();
    let configuration = oversized_plan.exact.fragments[0].placements[0].configuration[0].clone();
    oversized_plan.exact.fragments[0].placements[0].configuration =
        vec![configuration; MAX_RENDERER_PLAN_ITEMS + 1];
    assert_eq!(
        PatchbayFace::new(
            8,
            face.document.clone(),
            Some(oversized_plan),
            None,
            None,
            Vec::new(),
        ),
        Err(RendererProjectionError::PlanTooLarge)
    );

    let mut source = face.document.clone();
    source.source = "x".repeat(crate::MAX_PLOT_SOURCE_BYTES + 1);
    assert_eq!(
        PatchbayFace::new(8, source, None, None, None, Vec::new()),
        Err(RendererProjectionError::SourceTooLarge)
    );
}

#[test]
fn projection_rejects_a_drifting_document_plan_or_play_identity() {
    let (face, _) = living_projection();
    let mut plan = face.plan.clone().unwrap();
    plan.checked_plot_id = "checked/drift".into();
    assert_eq!(
        PatchbayFace::new(
            8,
            face.document.clone(),
            Some(plan),
            face.play.clone(),
            face.topology.clone(),
            face.routes.clone(),
        ),
        Err(RendererProjectionError::IdentityMismatch)
    );

    let mut play = face.play.clone().unwrap();
    play.plan_id = "plan/drift".into();
    assert_eq!(
        PatchbayFace::new(
            8,
            face.document,
            face.plan,
            Some(play),
            face.topology,
            face.routes,
        ),
        Err(RendererProjectionError::IdentityMismatch)
    );
}

#[test]
fn checked_attempted_edit_diagnostic_does_not_replace_valid_graph_identity() {
    let (face, _) = living_projection();
    let before = face.identities();
    let diagnostic = crate::EditorDiagnostic {
        code: "CND-PLOT-TEST",
        message: "malformed attempted edit".into(),
        span: face.document.checked.plots[0].source_span,
    };
    let stale = crate::AttemptedEditFace {
        revision: face.document.revision,
        source: "malformed".into(),
        diagnostics: vec![diagnostic.clone()],
    };
    assert_eq!(
        face.clone().with_attempted_edit(stale),
        Err(RendererProjectionError::InvalidAttemptedEdit)
    );
    let revised = face
        .with_attempted_edit(crate::AttemptedEditFace {
            revision: 1,
            source: "plot malformed {".into(),
            diagnostics: vec![diagnostic],
        })
        .unwrap();
    assert_eq!(revised.identities(), before);
    assert_eq!(revised.attempted_edit.unwrap().revision, 1);
}
