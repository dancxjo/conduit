use crate::RendererSnapshot;
use conduit_std_host::{StdHost, ThreadTimer};
use patchbay_model::{
    AttemptedEditPresentation, DistributedRouteDemo, FormEditor, PatchbayModel,
    PatchbayPresentation, PatchbayRequestId, PatchbayTopology, PlanDocument, PlayDocument,
};

pub fn demonstration_snapshot() -> Result<RendererSnapshot, String> {
    let editor = FormEditor::from_source(
        "examples/hello.conduit".into(),
        include_str!("../../../examples/hello.conduit").into(),
    )
    .map_err(|error| error.to_string())?;
    let expanded = editor
        .expand_form("hello")
        .map_err(|error| error.to_string())?;
    let mut host = StdHost::new();
    let plan = host
        .plan_expanded_local(&expanded)
        .map_err(|error| error.to_string())?;
    let plan_document = PlanDocument::from_plan(
        PatchbayRequestId::new("patchbay-html/plan").map_err(|error| format!("{error:?}"))?,
        &plan,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut output = Vec::with_capacity(4096);
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut ThreadTimer)
        .map_err(|error| error.to_string())?;
    let play_document =
        PlayDocument::from_report(&plan, &report).map_err(|error| format!("{error:?}"))?;
    let patchbay =
        PatchbayModel::with_identity("patchbay-html/host".into(), "patchbay-html/boot".into());
    let mut topology = PatchbayTopology::new(1).map_err(|error| error.to_string())?;
    topology
        .ingest(&patchbay.startup_snapshot())
        .map_err(|error| error.to_string())?;
    let route = DistributedRouteDemo::build()
        .map_err(|error| format!("{error:?}"))?
        .presentation()
        .clone();
    let malformed_source = format!(
        "{}\nform malformed {{\n",
        include_str!("../../../examples/hello.conduit")
    );
    let malformed =
        FormEditor::from_source("examples/hello.conduit".into(), malformed_source.clone())
            .map_err(|error| error.to_string())?
            .view();
    let presentation = PatchbayPresentation::new(
        1,
        editor.view(),
        Some(plan_document),
        Some(play_document),
        topology.current_report().cloned(),
        vec![route],
    )
    .map_err(|error| error.to_string())?
    .with_attempted_edit(AttemptedEditPresentation {
        revision: 1,
        source: malformed_source,
        diagnostics: malformed.checked.diagnostics,
    })
    .map_err(|error| error.to_string())?;
    Ok(RendererSnapshot::from_presentation(&presentation))
}
