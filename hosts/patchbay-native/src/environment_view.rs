//! Human-legible visual workspace for bounded authored physical-environment truth.

use crate::{
    canvas::SoftwareCanvas,
    gui::{GuiAction, HitTarget},
    gui_hit::HitShape,
    gui_primitives::{fill_rect, frame_rect, line, text, PixelRect},
};
use embedded_graphics::prelude::Point;
use patchbay_model::{AuthoredEnvironment, EnvironmentLinkKind, MachineProfile, PHOSPHOR_THEME};

const PART_WIDTH: u32 = 180;
const PART_HEIGHT: u32 = 112;

pub(super) struct EnvironmentViewContext<'a> {
    pub(super) selected: Option<&'a str>,
    pub(super) pending_link: Option<&'a (String, EnvironmentLinkKind)>,
    pub(super) observed: Option<&'a conduit_observatory::ObservatorySnapshot>,
    pub(super) prewake: Option<&'a patchbay_model::PrewakeController>,
}

pub(super) fn draw_environment(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    environment: &AuthoredEnvironment,
    view: EnvironmentViewContext<'_>,
) -> Vec<HitTarget> {
    let EnvironmentViewContext {
        selected,
        pending_link,
        observed,
        prewake,
    } = view;
    let mut canvas = SoftwareCanvas::new(pixels, width, height);
    let theme = &PHOSPHOR_THEME;
    fill_rect(
        &mut canvas,
        PixelRect {
            x: 0,
            y: 0,
            width: width as u32,
            height: height as u32,
        },
        theme.background,
    );
    text(
        &mut canvas,
        Point::new(18, 14),
        "AUTHORED ENVIRONMENT — SIMULATION INPUT",
        theme.emphasis,
    );
    text(
        &mut canvas,
        Point::new(18, 32),
        "DECLARED ≠ OBSERVED   NO PHYSICAL AUTHORITY",
        theme.focus,
    );
    text(
        &mut canvas,
        Point::new(18, 50),
        &format!(
            "{}  revision {}  parts {}  links {}",
            environment.environment_id,
            environment.revision,
            environment.parts.len(),
            environment.links.len()
        ),
        theme.text_secondary,
    );
    let mut targets = Vec::with_capacity(8 + environment.parts.len() * 2);
    for (index, (label, action)) in [
        ("+ PICO W", GuiAction::EnvironmentAdd(MachineProfile::PicoW)),
        (
            "+ RPI 5",
            GuiAction::EnvironmentAdd(MachineProfile::RaspberryPi5),
        ),
        (
            "+ LAPTOP",
            GuiAction::EnvironmentAdd(MachineProfile::LaptopLinux),
        ),
        ("SAVE", GuiAction::EnvironmentSave),
        (
            "LINK WIFI",
            GuiAction::EnvironmentLink(EnvironmentLinkKind::Wifi),
        ),
        (
            "LINK ETHERNET",
            GuiAction::EnvironmentLink(EnvironmentLinkKind::Ethernet),
        ),
        (
            "LINK USB",
            GuiAction::EnvironmentLink(EnvironmentLinkKind::Usb),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let bounds = PixelRect {
            x: 16,
            y: 86 + index as i32 * 34,
            width: 150,
            height: 26,
        };
        frame_rect(&mut canvas, bounds, theme.structure_secondary, 1);
        text(
            &mut canvas,
            Point::new(bounds.x + 8, bounds.y + 6),
            label,
            theme.text_primary,
        );
        targets.push(HitTarget {
            action,
            shape: HitShape::Rect(bounds),
        });
    }
    if let Some((part, kind)) = pending_link {
        text(
            &mut canvas,
            Point::new(18, 330),
            &format!("LINK {:?}: select peer for {part}", kind),
            theme.focus,
        );
    }
    if let Some(part) = selected {
        text(
            &mut canvas,
            Point::new(18, 352),
            &format!("SELECTED {part} — F2/ENTER rename, drag to move"),
            theme.text_secondary,
        );
    }
    if let Some(snapshot) = observed {
        text(
            &mut canvas,
            Point::new(18, 386),
            "OBSERVED LIVE HOSTS — SEPARATE / UNBOUND",
            theme.emphasis,
        );
        for (index, host) in snapshot.hosts.iter().take(8).enumerate() {
            text(
                &mut canvas,
                Point::new(18, 406 + index as i32 * 18),
                &format!(
                    "{} / {} / {}",
                    host.advertisement.host_id.as_str(),
                    host.advertisement.boot_id.as_str(),
                    host.advertisement.profile.as_str()
                ),
                theme.text_secondary,
            );
        }
    }
    if let Some(prewake) = prewake {
        let (state, plan, play) = match prewake.state() {
            patchbay_model::PrewakeState::Off => ("OFF", "none", "none"),
            patchbay_model::PrewakeState::Held { plan, .. } => {
                ("HOLD", plan.plan_id.as_str(), "not-started")
            }
            patchbay_model::PrewakeState::Auto { plan, play, .. } => (
                "AUTO",
                plan.plan_id.as_str(),
                play.active_play_ids
                    .first()
                    .map(|id| id.as_str())
                    .unwrap_or("none"),
            ),
        };
        text(
            &mut canvas,
            Point::new(184, 88),
            &format!("PREWAKE {state}  SIMULATED PLAN {plan}  PLAY {play}"),
            theme.focus,
        );
        text(
            &mut canvas,
            Point::new(184, 106),
            "SIMULATED HOST/BOOT/BASE/SIGN — ZERO PHYSICAL EFFECT AUTHORITY",
            theme.emphasis,
        );
        if let Some(error) = prewake.last_refusal() {
            text(
                &mut canvas,
                Point::new(184, 124),
                &format!("REFUSED {error:?}"),
                theme.focus,
            );
        }
        for (index, (label, action)) in [
            ("FORM VIEW (F3)", GuiAction::PrewakeToggleWorkspace),
            (
                if prewake.hold_enabled() {
                    "AUTO (F6)"
                } else {
                    "HOLD (F6)"
                },
                GuiAction::PrewakeToggleHold,
            ),
            ("RELEASE (F7)", GuiAction::PrewakeRelease),
            ("EXIT PREWAKE", GuiAction::PrewakeExit),
        ]
        .into_iter()
        .enumerate()
        {
            let bounds = PixelRect {
                x: 184 + index as i32 * 158,
                y: 136,
                width: 148,
                height: 24,
            };
            frame_rect(&mut canvas, bounds, theme.structure_secondary, 1);
            text(
                &mut canvas,
                Point::new(bounds.x + 6, bounds.y + 5),
                label,
                theme.text_primary,
            );
            targets.push(HitTarget {
                action,
                shape: HitShape::Rect(bounds),
            });
        }
        let plan = match prewake.state() {
            patchbay_model::PrewakeState::Held { plan, .. }
            | patchbay_model::PrewakeState::Auto { plan, .. } => Some(plan),
            patchbay_model::PrewakeState::Off => None,
        };
        if let Some(plan) = plan {
            let placements = plan
                .fragments
                .iter()
                .map(|fragment| fragment.placements.len())
                .sum::<usize>();
            let lines = plan
                .fragments
                .iter()
                .map(|fragment| fragment.connections.len())
                .sum::<usize>();
            let resources = plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .map(|placement| placement.resources.len())
                .sum::<usize>();
            text(
                &mut canvas,
                Point::new(184, 500),
                &format!(
                    "EXACT PLAN fragments={} placements={} resources={} Lines={}",
                    plan.fragments.len(),
                    placements,
                    resources,
                    lines
                ),
                theme.text_secondary,
            );
            for (index, placement) in plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .take(4)
                .enumerate()
            {
                text(
                    &mut canvas,
                    Point::new(184, 518 + index as i32 * 18),
                    &format!(
                        "{} -> host={} boot={} implementation={}",
                        placement.gear_id.as_str(),
                        placement.host_id.as_str(),
                        placement.boot_id.as_str(),
                        placement.implementation_id.as_str()
                    ),
                    theme.text_secondary,
                );
            }
        }
        if let patchbay_model::PrewakeState::Auto { play, .. } = prewake.state() {
            text(
                &mut canvas,
                Point::new(184, 596),
                &format!(
                    "SIMULATED TERMINAL {:?}  kernel Signs={}  output-bytes={}",
                    play.terminal,
                    play.kernel_sign.len(),
                    play.output.len()
                ),
                theme.emphasis,
            );
        }
    }
    for link in &environment.links {
        let Some(left) = environment
            .parts
            .iter()
            .find(|part| part.part_id == link.left_part_id)
        else {
            continue;
        };
        let Some(right) = environment
            .parts
            .iter()
            .find(|part| part.part_id == link.right_part_id)
        else {
            continue;
        };
        let start = Point::new(
            left.x + PART_WIDTH as i32 / 2,
            left.y + PART_HEIGHT as i32 / 2,
        );
        let end = Point::new(
            right.x + PART_WIDTH as i32 / 2,
            right.y + PART_HEIGHT as i32 / 2,
        );
        line(&mut canvas, start, end, theme.structure_secondary);
        text(
            &mut canvas,
            Point::new((start.x + end.x) / 2, (start.y + end.y) / 2),
            &format!("{:?}", link.kind).to_ascii_uppercase(),
            theme.text_secondary,
        );
    }
    for part in &environment.parts {
        let bounds = PixelRect {
            x: part.x,
            y: part.y,
            width: PART_WIDTH,
            height: PART_HEIGHT,
        };
        fill_rect(&mut canvas, bounds, theme.surface);
        frame_rect(
            &mut canvas,
            bounds,
            if selected == Some(part.part_id.as_str()) {
                theme.focus
            } else {
                theme.structure_primary
            },
            if selected == Some(part.part_id.as_str()) {
                2
            } else {
                1
            },
        );
        text(
            &mut canvas,
            Point::new(part.x + 10, part.y + 9),
            &part.name,
            theme.text_primary,
        );
        text(
            &mut canvas,
            Point::new(part.x + 10, part.y + 27),
            part.profile.human_name(),
            theme.emphasis,
        );
        text(
            &mut canvas,
            Point::new(part.x + 10, part.y + 45),
            &format!(
                "compute {}  memory {} KiB",
                part.resources.compute_units,
                part.resources.memory_bytes / 1024
            ),
            theme.text_secondary,
        );
        text(
            &mut canvas,
            Point::new(part.x + 10, part.y + 63),
            &format!("{:?}", part.resources.connectivity),
            theme.text_secondary,
        );
        text(
            &mut canvas,
            Point::new(part.x + 10, part.y + 82),
            "MODELED / NOT LIVE",
            theme.focus,
        );
        targets.push(HitTarget {
            action: GuiAction::EnvironmentSelect(part.part_id.clone()),
            shape: HitShape::Rect(bounds),
        });
        let remove = PixelRect {
            x: part.x + PART_WIDTH as i32 - 54,
            y: part.y + 7,
            width: 46,
            height: 20,
        };
        frame_rect(&mut canvas, remove, theme.structure_secondary, 1);
        text(
            &mut canvas,
            Point::new(remove.x + 5, remove.y + 4),
            "REMOVE",
            theme.text_primary,
        );
        targets.push(HitTarget {
            action: GuiAction::EnvironmentRemove(part.part_id.clone()),
            shape: HitShape::Rect(remove),
        });
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::BACKGROUND;
    use patchbay_model::{AuthoredLink, AuthoredPart};

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
            },
        );
        assert!(targets
            .iter()
            .any(|target| target.action == GuiAction::EnvironmentSave));
        assert!(targets.iter().any(|target| matches!(
            target.action,
            GuiAction::EnvironmentAdd(MachineProfile::PicoW)
        )));
        assert!(targets.iter().any(
            |target| matches!(&target.action, GuiAction::EnvironmentRemove(id) if id == "pico")
        ));
        assert!(pixels.contains(&PHOSPHOR_THEME.focus.packed_rgb()));
        let projection = environment.simulation_projection().unwrap();
        assert!(!projection.provenance.observed_live_truth);
        assert!(!projection.provenance.authority_granted);
    }
}
