//! Legacy product demonstration over the extracted Patchbay conformance plan.

use conduit_core::SignId;

pub fn recursive_plot_demonstration() -> Result<conduit_presentation::Presentation, String> {
    let proof = conduit_patchbay_workbench_conformance::patchbay_mask_plans()?;
    let (startup, profile) = conduit_patchbay_workbench_conformance::patchbay_catalogs()?;
    let editor = crate::PlotEditor::from_source_with_catalogs(
        "patchbay-recursive-plot.conduit".into(),
        "plot patchbay-capstone {\n subject: text/literal(\"Gear demo with typed Ports and one Cord\")\n canvas: presentation/patchbay\n subject >> canvas.subject\n}\n".into(),
        startup.clone(),
        profile,
    )
    .map_err(|error| error.to_string())?;
    let mut graph = crate::PatchbayGraph::from_expanded(&proof.recursive_expanded)
        .map_err(|error| error.to_string())?;
    for back in &proof.recursive_expanded.realization_backs {
        let front =
            conduit_patchbay_workbench_conformance::reviewed_patchbay_back_front(back, &startup)?;
        let projection = crate::project_recursive_plot_gear(
            &proof.recursive_expanded,
            &back.invocation_path,
            front,
            false,
        )
        .map_err(|error| format!("recursive Plot projection: {error:?}"))?;
        graph
            .admit_recursive_plot(&projection)
            .map_err(|error| error.to_string())?;
    }
    let request = crate::PatchbayRequestId::new("patchbay/recursive-plot-plan")
        .map_err(|error| format!("{error:?}"))?;
    let plan = crate::PlanDocument::from_plan(request, &proof.recursive)
        .map_err(|error| format!("{error:?}"))?;
    let body = conduit_body::Body::born(
        proof.recursive.source_document_id.clone(),
        proof.recursive.checked_plot_id.clone(),
        0,
        SignId::from("patchbay/recursive-plot/born"),
    )
    .map_err(|error| error.to_string())?;
    let (body, wake) = body
        .wake(1, SignId::from("patchbay/recursive-plot/woke"))
        .map_err(|error| error.to_string())?;
    let wake = wake
        .plan_ready(
            &proof.recursive,
            SignId::from("patchbay/recursive-plot/planned"),
        )
        .map_err(|error| error.to_string())?;
    let presentation =
        crate::PatchbayPresentation::new(1, editor.view(), Some(plan), None, None, Vec::new())
            .map_err(|error| error.to_string())?
            .with_graph(graph)
            .map_err(|error| error.to_string())?
            .to_portable(&body, &wake)
            .map_err(|error| error.to_string())?;
    let body_subject = format!("body/{}", body.body_id.as_str());
    let mut subjects = presentation.subjects;
    subjects.push(conduit_presentation::PresentationSubject {
        identity: body_subject,
        role: conduit_presentation::PresentationRole::Body,
        name: "Recursive Plot demonstration Body".into(),
    });
    conduit_presentation::Presentation::new_with_semantics(
        presentation.revision,
        presentation.basis,
        subjects,
        presentation.relationships,
        presentation.properties,
        presentation.text,
        presentation.actions,
        presentation.disclosures,
    )
    .map_err(|error| error.to_string())
}
