//! Deterministic documentary Body used only by browser proof entrances.

use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyGraduationChoice,
    BodyGraduationEvidence, BodyMembership, BodyWorkset, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::{bind_sign, BootId, HostId, ImplementationId, OfferGeneration, PlanId, SignId};
use conduit_patchbay_workbench::PlotCandidate;

use crate::{
    body_workbench_snapshot_with_plots, BodyWorkbenchError, BrowserBodyWorkbenchEntrance,
    RendererSnapshot,
};

/// Product entrances use caller-supplied serialized evidence instead.
pub fn body_workbench_fixture_snapshot(
    hosted: bool,
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    let plots = body_workbench_fixture_plots()?;
    body_workbench_fixture_snapshot_with_plots(hosted, &plots)
}

pub fn body_workbench_fixture_plots() -> Result<Vec<PlotCandidate>, BodyWorkbenchError> {
    Ok(vec![
        reviewed_plot(
            "Hello",
            "plots/hello/main.conduit",
            include_str!("../../../../plots/hello/main.conduit"),
            "hello",
            5,
        )?,
        reviewed_plot(
            "Greet",
            "plots/greet/main.conduit",
            include_str!("../../../../plots/greet/main.conduit"),
            "greet",
            6,
        )?,
        reviewed_plot(
            "Count",
            "plots/count/main.conduit",
            include_str!("../../../../plots/count/main.conduit"),
            "count",
            7,
        )?,
        reviewed_plot(
            "Desk Telegraph",
            "plots/desk-telegraph/main.conduit",
            include_str!("../../../../plots/desk-telegraph/main.conduit"),
            "desk_telegraph",
            8,
        )?,
        reviewed_plot(
            "Memory Lantern",
            "plots/memory-lantern/main.conduit",
            include_str!("../../../../plots/memory-lantern/main.conduit"),
            "memory_lantern",
            9,
        )?,
    ])
}

fn body_workbench_fixture_snapshot_with_plots(
    hosted: bool,
    available: &[PlotCandidate],
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    const PLAN: &str = "plan/roseau-hosted-patchbay";
    const IMPLEMENTATION: &str = "browser/patchbay-surface@1";
    let host = HostId::from("host/roseau");
    let boot = BootId::from("boot/roseau/1");
    let body = Body::born_with_plots(
        BodyWorkset::from_plots([
            ResidentPlot::new(
                available[3].source_document_id.clone(),
                available[3].checked_plot_id.clone(),
            ),
            ResidentPlot::new(
                available[4].source_document_id.clone(),
                available[4].checked_plot_id.clone(),
            ),
        ])
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?,
        1,
        bind_sign(&host, &boot, None, 1).sign_id,
    )
    .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let mut membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let part = PartId::bind(&body.body_id, "roseau/here", 1)
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let proof = MembershipProofId::bind("proof/roseau/here")
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            bind_sign(&host, &boot, None, 2).sign_id,
        )
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let joined = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            bind_sign(&host, &boot, None, 3).sign_id,
        )
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let mut evidence = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone())
            .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?,
        "Roseau".into(),
    )
    .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    evidence
        .append_membership_events(membership, &[(admitted, 2), (joined, 3)])
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let choice = if hosted {
        BodyGraduationChoice::HostedReader
    } else {
        BodyGraduationChoice::ExternalReader
    };
    evidence
        .graduate(BodyGraduationEvidence {
            body_id: evidence.body_id.clone(),
            sequence: 4,
            sign_id: SignId::from("sign/roseau/graduated"),
            choice,
            reader_plan_id: hosted.then(|| PlanId::from(PLAN)),
            reader_implementation_id: hosted.then(|| ImplementationId::from(IMPLEMENTATION)),
        })
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let encoded = serde_json::to_vec(&evidence).map_err(BodyWorkbenchError::Encode)?;
    let entrance = if hosted {
        BrowserBodyWorkbenchEntrance::Hosted {
            plan_id: PLAN.into(),
            implementation_id: IMPLEMENTATION.into(),
        }
    } else {
        BrowserBodyWorkbenchEntrance::ExternalReader
    };
    body_workbench_snapshot_with_plots(1, &encoded, entrance, available)
}

fn reviewed_plot(
    label: &str,
    source_name: &str,
    source: &str,
    plot_name: &str,
    freshness: u64,
) -> Result<PlotCandidate, BodyWorkbenchError> {
    PlotCandidate::from_source_plot(
        label,
        source_name,
        source,
        plot_name,
        "reviewed canonical fixture Plot",
        SignId::from(format!("sign/{}-reviewed", label.to_lowercase())),
        freshness,
    )
    .map_err(BodyWorkbenchError::Projection)
}
