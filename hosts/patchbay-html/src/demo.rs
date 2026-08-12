use crate::RendererSnapshot;
use conduit_core::{BootId, HostId, SignId};
use patchbay_model::{RendererAdapterIdentity, RendererAdapterKind, RendererExecution};

pub fn demonstration_snapshot() -> Result<RendererSnapshot, String> {
    let (presentation, parts) = patchbay_model::portable_demonstration_with_parts()?;
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
    Ok(snapshot)
}
