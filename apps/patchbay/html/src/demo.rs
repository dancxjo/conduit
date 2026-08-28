use crate::RendererSnapshot;
use conduit_core::{BootId, HostId, SignId};
use patchbay_model::{
    PatchbayNavigationProjection, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};

pub fn demonstration_snapshot() -> Result<RendererSnapshot, String> {
    let (presentation, parts) = patchbay_model::portable_demonstration_with_parts_and_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/host"),
            boot_id: BootId::from("patchbay-html/boot"),
            target_subject: "patchbay-html/document-0".into(),
        },
        SignId::from("patchbay-html/manifestation-prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    snapshot
        .attach_parts(parts)
        .map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub fn llm_documentary_snapshot() -> Result<RendererSnapshot, String> {
    let presentation = patchbay_model::llm_documentary_presentation_with_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/llm-documentary"),
            boot_id: BootId::from("patchbay-html/llm-documentary/boot"),
            target_subject: "patchbay-html/llm-documentary/document".into(),
        },
        SignId::from("patchbay-html/llm-documentary/prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub fn llm_embodiment_snapshot(stage: usize) -> Result<RendererSnapshot, String> {
    let presentation = patchbay_model::llm_embodiment_documentary_presentations()
        .map_err(|error| format!("{error:?}"))?
        .into_iter()
        .nth(stage)
        .ok_or("LLM embodiment stage must be 0, 1, or 2")?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/llm-embodiment"),
            boot_id: BootId::from("patchbay-html/llm-embodiment/boot"),
            target_subject: format!("patchbay-html/llm-embodiment/{stage}"),
        },
        SignId::from(format!("patchbay-html/llm-embodiment/{stage}/prepared")),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub fn text_lab_split_snapshot(base: &str) -> Result<RendererSnapshot, String> {
    let explanation = patchbay_model::text_lab_split_explanation(base)?;
    text_lab_snapshot(explanation)
}

pub fn text_lab_split_loss_snapshot(
    base: &str,
    receipt: &conduit_semantic_catalog::TextLabLineLossReceipt,
) -> Result<RendererSnapshot, String> {
    text_lab_snapshot(patchbay_model::text_lab_split_loss_explanation(
        base, receipt,
    )?)
}

fn text_lab_snapshot(
    explanation: patchbay_model::TextLabSplitExplanation,
) -> Result<RendererSnapshot, String> {
    let execution = RendererExecution::prepare(
        explanation.presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/text-lab"),
            boot_id: BootId::from("patchbay-html/text-lab/boot"),
            target_subject: "patchbay-html/text-lab/document".into(),
        },
        SignId::from("patchbay-html/text-lab/prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    snapshot
        .attach_navigation(explanation.navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}
