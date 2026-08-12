use crate::{
    environment_view::{draw_environment, EnvironmentViewContext},
    gui::GuiAction,
    interaction_status::{InteractionStatusChannel, InteractionStatusCode, InteractionStatusLevel},
    render::BACKGROUND,
};
use patchbay_model::{
    AuthoredEnvironment, AuthoredLink, AuthoredPart, EnvironmentLinkKind, MachineProfile,
    PHOSPHOR_THEME,
};

#[test]
fn visual_workspace_exposes_profiles_parts_links_and_truth_boundary() {
    let mut environment = AuthoredEnvironment::new("workbench").unwrap();
    let mut pico = AuthoredPart::reviewed("pico", "Pico", MachineProfile::PicoW);
    pico.x = 220;
    pico.y = 160;
    let mut laptop = AuthoredPart::reviewed("laptop", "Forebrain", MachineProfile::LaptopLinux);
    laptop.x = 520;
    laptop.y = 160;
    environment.add_part(pico).unwrap();
    environment.add_part(laptop).unwrap();
    environment
        .add_link(AuthoredLink {
            link_id: "wifi".into(),
            left_part_id: "pico".into(),
            right_part_id: "laptop".into(),
            kind: EnvironmentLinkKind::Wifi,
        })
        .unwrap();
    let mut pixels = vec![BACKGROUND; 1000 * 600];
    let targets = draw_environment(
        &mut pixels,
        1000,
        600,
        &environment,
        EnvironmentViewContext {
            selected: Some("pico"),
            pending_link: None,
            observed: None,
            prewake: None,
            drag: None,
            status: None,
        },
    );
    assert!(targets
        .iter()
        .any(|target| target.action == GuiAction::EnvironmentSave));
    assert!(targets.iter().any(|target| matches!(
        target.action,
        GuiAction::EnvironmentAdd(MachineProfile::PicoW)
    )));
    assert!(targets
        .iter()
        .any(|target| matches!(&target.action, GuiAction::EnvironmentRemove(id) if id == "pico")));
    assert!(pixels.contains(&PHOSPHOR_THEME.focus.packed_rgb()));
    let baseline = pixels.clone();
    let mut status = InteractionStatusChannel::default();
    status.publish(
        InteractionStatusLevel::Information,
        InteractionStatusCode::Gesture,
        "Moving environment part pico",
    );
    draw_environment(
        &mut pixels,
        1000,
        600,
        &environment,
        EnvironmentViewContext {
            selected: Some("pico"),
            pending_link: None,
            observed: None,
            prewake: None,
            drag: Some(("pico", (700.0, 350.0))),
            status: status.current(),
        },
    );
    assert_ne!(pixels, baseline);
    let projection = environment.simulation_projection().unwrap();
    assert!(!projection.provenance.observed_live_truth);
    assert!(!projection.provenance.authority_granted);
}
