//! One living, bounded semantic input shared by renderer adapter proofs.

use conduit_body::Body;
use conduit_core::{bind_active_play, BootId, HostId, OfferGeneration, SignId};
use conduit_presentation::Presentation;
use conduit_std_host::{StdHost, StdHostConfig, ThreadTimer};

use crate::{
    AttemptedEditPresentation, DistributedRouteDemo, FormEditor, PatchbayModel,
    PatchbayPresentation, PatchbayRequestId, PatchbayTopology, PlanDocument, PlayDocument,
};

pub fn portable_demonstration() -> Result<Presentation, String> {
    let editor = FormEditor::from_source(
        "examples/hello.conduit".into(),
        include_str!("../../../examples/hello.conduit").into(),
    )
    .map_err(|error| error.to_string())?;
    let expanded = editor
        .expand_form("hello")
        .map_err(|error| error.to_string())?;
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("patchbay-portable/host"),
        boot_id: BootId::from("patchbay-portable/boot"),
        offer_generation: OfferGeneration(1),
    });
    let host_id = host.advertisement().host_id.clone();
    let boot_id = host.advertisement().boot_id.clone();
    let plan = host
        .plan_expanded_local(&expanded)
        .map_err(|error| error.to_string())?;
    let plan_document = PlanDocument::from_plan(
        PatchbayRequestId::new("patchbay/portable-plan").map_err(|error| format!("{error:?}"))?,
        &plan,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut output = Vec::with_capacity(4096);
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut ThreadTimer)
        .map_err(|error| error.to_string())?;
    let play_document =
        PlayDocument::from_report(&plan, &report).map_err(|error| format!("{error:?}"))?;
    let patchbay = PatchbayModel::with_identity("patchbay/host".into(), "patchbay/boot".into());
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
    let projection = PatchbayPresentation::new(
        1,
        editor.view(),
        Some(plan_document),
        Some(play_document),
        topology.current_report().cloned(),
        vec![route],
    )
    .map_err(|error| error.to_string())?
    .with_graph(crate::PatchbayGraph::from_expanded(&expanded).map_err(|error| error.to_string())?)
    .map_err(|error| error.to_string())?
    .with_attempted_edit(AttemptedEditPresentation {
        revision: 1,
        source: malformed_source,
        diagnostics: malformed.checked.diagnostics,
    })
    .map_err(|error| error.to_string())?;
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
        1,
        SignId::from("patchbay/bornd"),
    )
    .map_err(|error| error.to_string())?;
    let (body, wake) = body
        .wake(1, SignId::from("patchbay/woke"))
        .map_err(|error| error.to_string())?;
    let wake = wake
        .plan_ready(&plan, SignId::from("patchbay/planned"))
        .map_err(|error| error.to_string())?;
    let active_play = bind_active_play(&plan.plan_id, &host_id, &boot_id, 0);
    let wake = wake
        .play_started(&active_play, SignId::from("patchbay/playing"))
        .map_err(|error| error.to_string())?;
    projection
        .to_portable(&body, &wake)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documentary_fixture_keeps_exact_semantic_identities() {
        let first = portable_demonstration().unwrap();
        let second = portable_demonstration().unwrap();
        assert_eq!(first.identity, second.identity);
        assert_eq!(first.basis.plan_id, second.basis.plan_id);
        assert_eq!(first.basis.active_play_id, second.basis.active_play_id);
        assert_eq!(first.subjects, second.subjects);
    }
}
