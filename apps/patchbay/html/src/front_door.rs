//! Public HTML entrance backed by one canonical live local Body session.

use crate::RendererSnapshot;
use conduit_core::{BootId, HostId, SignId};
use patchbay_model::{
    GearPalette, LocalFrontDoor, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
    ZeroBodyFrontDoor,
};

use crate::transport_types::{
    BrowserAuthoring, BrowserPaletteConfiguration, BrowserPaletteEntry, BrowserPalettePort,
};

pub fn front_door_snapshot() -> Result<RendererSnapshot, String> {
    let session =
        ZeroBodyFrontDoor::fresh(std::sync::Arc::new(patchbay_hosted::HostedPatchbayAdapter))?;
    snapshot_for_zero_body_front_door(&session)
}

pub(crate) fn snapshot_for_zero_body_front_door(
    session: &ZeroBodyFrontDoor,
) -> Result<RendererSnapshot, String> {
    let projection = session.project()?;
    let navigation = projection.navigation;
    let execution = RendererExecution::prepare(
        projection.presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/front-door"),
            boot_id: BootId::from("patchbay-html/front-door/boot-1"),
            target_subject: "patchbay-html/front-door/document".into(),
        },
        SignId::from("patchbay-html/front-door/prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    if let Some(document) = session.opened_seed_document() {
        snapshot
            .attach_authoring(browser_authoring(&document)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(snapshot)
}

pub(crate) fn snapshot_for_front_door(
    session: &LocalFrontDoor,
) -> Result<RendererSnapshot, String> {
    let projection = session.project()?;
    let navigation = projection.navigation;
    let execution = RendererExecution::prepare(
        projection.presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/front-door"),
            boot_id: BootId::from("patchbay-html/front-door/boot-1"),
            target_subject: "patchbay-html/front-door/document".into(),
        },
        SignId::from("patchbay-html/front-door/prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    snapshot
        .attach_parts(projection.parts)
        .map_err(|error| error.to_string())?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

fn browser_authoring(
    document: &patchbay_model::FormDocumentView,
) -> Result<BrowserAuthoring, String> {
    let source_document_id = document
        .checked
        .source_document_id
        .as_ref()
        .ok_or("opened Seed source is unchecked")?;
    let graph =
        patchbay_model::FormEditor::from_source(document.path.clone(), document.source.clone())
            .map_err(|error| error.to_string())?
            .patchbay_graph_for_authoring(&document.open_form)
            .map_err(|error| error.to_string())?;
    let palette = GearPalette::standard()
        .map_err(|error| format!("Gear palette: {error:?}"))?
        .entries()
        .iter()
        .map(|entry| BrowserPaletteEntry {
            kind_id: entry.kind_id.as_str().into(),
            name: entry.plain_name.clone(),
            summary: entry.summary.clone(),
            category: entry.category.label().into(),
            tags: entry.tags.iter().map(|tag| (*tag).into()).collect(),
            icon: format!("{:?}", entry.icon),
            inputs: entry.inputs.iter().map(browser_palette_port).collect(),
            outputs: entry.outputs.iter().map(browser_palette_port).collect(),
            configuration: entry
                .configuration
                .iter()
                .map(|field| BrowserPaletteConfiguration {
                    key: field.key.clone(),
                    default_value: field.default_value.clone(),
                    rule: field.rule.clone(),
                })
                .collect(),
        })
        .collect();
    Ok(BrowserAuthoring {
        source_document_id: source_document_id.as_str().into(),
        source_revision: document.revision,
        saved_revision: document.saved_revision,
        expanded_form_id: graph.expanded_form_id.as_str().into(),
        source_path: document.path.display().to_string(),
        palette,
    })
}

fn browser_palette_port(port: &conduit_core::PortDescriptor) -> BrowserPalettePort {
    BrowserPalettePort {
        identity: port.port_id.as_str().into(),
        info: port.value_kind.as_str().into(),
        temporal: format!("{:?}", port.temporal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::PresentationRole;

    #[test]
    fn public_front_door_truthfully_begins_without_a_body() {
        let snapshot = front_door_snapshot().unwrap();
        assert!(snapshot.parts.is_none());
        assert!(snapshot.presentation.basis.body_id.is_none());
        assert!(snapshot.entrance.body_id.is_none());
        assert_eq!(
            snapshot
                .presentation
                .subjects
                .iter()
                .find(|subject| {
                    Some(subject.identity.as_str()) == snapshot.entrance.selected_subject.as_deref()
                })
                .map(|subject| subject.role),
            Some(PresentationRole::Host)
        );
        assert!(snapshot.presentation.basis.plan_id.is_none());
        assert!(snapshot
            .presentation
            .subjects
            .iter()
            .any(|subject| subject.role == PresentationRole::Seed));
        assert!(!snapshot
            .presentation
            .subjects
            .iter()
            .any(|subject| subject.role == PresentationRole::Body));
        assert_eq!(snapshot.presentation.actions.len(), 2);
        assert_eq!(snapshot.presentation.disclosures.len(), 2);
        let seed = snapshot
            .presentation
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Seed)
            .unwrap();
        let actions = snapshot
            .presentation
            .actions
            .iter()
            .filter(|action| action.target == seed.identity)
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].label, "Open");
        assert_eq!(actions[1].label, "Birth");
    }
}
