use crate::RendererSnapshot;
use conduit_core::{BootId, ClueId, HostId};
use patchbay_model::{RendererAdapterIdentity, RendererAdapterKind, RendererExecution};

pub fn demonstration_snapshot() -> Result<RendererSnapshot, String> {
    let execution = RendererExecution::prepare(
        patchbay_model::portable_demonstration()?,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/host"),
            boot_id: BootId::from("patchbay-html/boot"),
            target_subject: "patchbay-html/document-0".into(),
        },
        ClueId::from("patchbay-html/manifestation-prepared"),
    )
    .map_err(|error| error.to_string())?;
    RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())
}
