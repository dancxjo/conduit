//! Public HTML entrance backed by one canonical live local body session.

use crate::RendererSnapshot;
use conduit_core::{BootId, HostId, SignId};
use conduit_patchbay_workbench::{
    prepare_renderer_execution, LocalFrontDoor, RendererAdapterIdentity, RendererAdapterKind,
    ZeroBodyFrontDoor,
};
use patchbay_application::{ONE_PLOT_TWO_FACES_BOOT_ID, ONE_PLOT_TWO_FACES_HOST_ID};

use crate::transport_types::{
    BrowserAuthoring, BrowserGearConfiguration, BrowserPaletteConfiguration, BrowserPaletteEntry,
    BrowserPalettePort,
};

pub fn front_door_snapshot() -> Result<RendererSnapshot, String> {
    let session =
        ZeroBodyFrontDoor::fresh(std::sync::Arc::new(patchbay_hosted::HostedPatchbayAdapter))?;
    snapshot_for_zero_body_front_door(&session)
}

pub fn one_plot_two_fronts_snapshot() -> Result<RendererSnapshot, String> {
    let mut session = ZeroBodyFrontDoor::with_identity(
        std::sync::Arc::new(patchbay_hosted::HostedPatchbayAdapter),
        HostId::from(ONE_PLOT_TWO_FACES_HOST_ID),
        BootId::from(ONE_PLOT_TWO_FACES_BOOT_ID),
    )?;
    let plot = session
        .plot_ids()
        .into_iter()
        .next()
        .ok_or("two-fronts entrance has no reviewed plot")?;
    session.open_plot(&plot, session.revision())?;
    snapshot_for_zero_body_front_door(&session)
}

pub(crate) fn snapshot_for_zero_body_front_door(
    session: &ZeroBodyFrontDoor,
) -> Result<RendererSnapshot, String> {
    let projection = session.project()?;
    let navigation = projection.navigation;
    let execution = prepare_renderer_execution(
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
    if let Some(editor) = session.opened_plot_editor() {
        snapshot
            .attach_authoring(browser_authoring(&editor)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(snapshot)
}

pub(crate) fn snapshot_for_front_door(
    session: &LocalFrontDoor,
) -> Result<RendererSnapshot, String> {
    let projection = session.project()?;
    let navigation = projection.navigation;
    let execution = prepare_renderer_execution(
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
    editor: &conduit_patchbay_workbench::PlotEditor,
) -> Result<BrowserAuthoring, String> {
    let document = editor.view();
    let source_document_id = document
        .checked
        .source_document_id
        .as_ref()
        .ok_or("opened Plot source is unchecked")?;
    let graph = editor
        .patchbay_graph_for_authoring(&document.open_plot)
        .map_err(|error| error.to_string())?;
    let palette = editor
        .authoring_catalog()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|kind| {
            let entry = kind.contract;
            BrowserPaletteEntry {
                kind_id: entry.kind_id.as_str().into(),
                kind_contract_revision: entry.kind_contract_revision,
                front: entry.front,
                limits: entry.limits,
                authorable: kind.authorable,
                startup_parameters: kind.startup_parameters,
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
            }
        })
        .collect();
    Ok(BrowserAuthoring {
        source_document_id: source_document_id.as_str().into(),
        checked_plot_id: graph.checked_plot_id.as_str().into(),
        source_revision: document.revision,
        saved_revision: document.saved_revision,
        expanded_plot_id: graph.expanded_plot_id.as_str().into(),
        source_path: document.path.display().to_string(),
        palette,
        configuration: graph
            .gears
            .iter()
            .flat_map(|gear| {
                gear.controls
                    .iter()
                    .map(|control| BrowserGearConfiguration {
                        gear_identity: gear.identity.clone(),
                        key: control.key.clone(),
                        value: control.value.clone(),
                    })
            })
            .collect(),
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
                .map(|subject| subject.role.clone()),
            Some(PresentationRole::Host)
        );
        assert!(snapshot.presentation.basis.plan_id.is_none());
        assert!(snapshot
            .presentation
            .subjects
            .iter()
            .any(|subject| subject.role == PresentationRole::Plot));
        assert!(!snapshot
            .presentation
            .subjects
            .iter()
            .any(|subject| subject.role == PresentationRole::Body));
        let plot_count = snapshot
            .presentation
            .subjects
            .iter()
            .filter(|subject| subject.role == PresentationRole::Plot)
            .count();
        assert_eq!(snapshot.presentation.actions.len(), 2 * plot_count + 1);
        assert_eq!(snapshot.presentation.disclosures.len(), plot_count + 1);
        let plot = snapshot
            .presentation
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Plot)
            .unwrap();
        let actions = snapshot
            .presentation
            .actions
            .iter()
            .filter(|action| action.target == plot.identity)
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].name, "Open");
        assert_eq!(actions[1].name, "Birth");
        let host = snapshot
            .presentation
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Host)
            .unwrap();
        let creche_birth = snapshot
            .presentation
            .actions
            .iter()
            .find(|action| action.target == host.identity)
            .unwrap();
        assert_eq!(creche_birth.name, "Name Body and choose Plots");
        assert!(creche_birth.availability.is_available());
    }
}
