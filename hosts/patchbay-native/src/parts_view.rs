//! Native drawing for the canonical human-first Parts projection.

use crate::{
    gui_hit::{GuiAction, HitShape, HitTarget},
    gui_primitives::{frame_rect, text, PixelRect},
};
use conduit_body::{CandidateId, PartId};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{PartPresentationState, PartsAction, PartsView, PatchbayTheme};

pub(super) struct PartsSelection<'a> {
    pub(super) part: Option<&'a PartId>,
    pub(super) candidate: Option<&'a CandidateId>,
    pub(super) pending_revoke: Option<&'a PartId>,
}

pub(super) fn draw_parts<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    view: &PartsView,
    selection: PartsSelection<'_>,
    canvas: PixelRect,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let PartsSelection {
        part: selected,
        candidate: selected_candidate,
        pending_revoke,
    } = selection;
    let left = canvas.x + 28;
    let top = canvas.y + 24;
    text(target, Point::new(left, top), "BODY", theme.emphasis);
    text(
        target,
        Point::new(left + 58, top),
        view.body_id.as_str(),
        theme.text_primary,
    );
    text(
        target,
        Point::new(left, top + 26),
        if view.awake { "AWAKE" } else { "LULLED" },
        theme.focus,
    );
    text(target, Point::new(left, top + 62), "PARTS", theme.emphasis);
    for (index, row) in view.parts.iter().enumerate() {
        let y = top + 88 + index as i32 * 54;
        let bounds = PixelRect {
            x: left,
            y: y - 8,
            width: canvas.width.saturating_sub(56),
            height: 46,
        };
        frame_rect(
            target,
            bounds,
            if selected == Some(&row.details.part_id) {
                theme.focus
            } else {
                theme.structure_secondary
            },
            if selected == Some(&row.details.part_id) {
                2
            } else {
                1
            },
        );
        let state = match row.state {
            PartPresentationState::Here => "HERE",
            PartPresentationState::Attached => "ATTACHED",
            PartPresentationState::Offline => "OFFLINE",
        };
        text(
            target,
            Point::new(left + 8, y),
            &row.label,
            theme.text_primary,
        );
        text(target, Point::new(left + 190, y), state, theme.emphasis);
        let mut truth = if row.available {
            "AVAILABLE".to_owned()
        } else {
            "NOT AVAILABLE".to_owned()
        };
        if row.in_plan {
            truth.push_str("  IN PLAN");
        }
        if row.playing {
            truth.push_str("  PLAYING");
        }
        text(
            target,
            Point::new(left + 8, y + 20),
            &truth,
            theme.text_secondary,
        );
        targets.push(HitTarget {
            action: GuiAction::InspectPart(row.details.part_id.clone()),
            shape: HitShape::Rect(bounds),
        });
    }
    let candidates_y = top + 102 + view.parts.len() as i32 * 54;
    text(
        target,
        Point::new(left, candidates_y),
        "WANTS TO JOIN",
        theme.emphasis,
    );
    for (index, row) in view.wants_to_join.iter().enumerate() {
        let y = candidates_y + 28 + index as i32 * 54;
        let bounds = PixelRect {
            x: left,
            y: y - 8,
            width: canvas.width.saturating_sub(56),
            height: 46,
        };
        let is_selected = selected_candidate == Some(&row.candidate_id);
        frame_rect(
            target,
            bounds,
            if is_selected {
                theme.focus
            } else {
                theme.structure_secondary
            },
            if is_selected { 2 } else { 1 },
        );
        text(
            target,
            Point::new(left + 8, y),
            &row.label,
            theme.text_primary,
        );
        text(
            target,
            Point::new(left + 190, y),
            "ADMISSION REQUIRED",
            theme.emphasis,
        );
        text(
            target,
            Point::new(left + 8, y + 20),
            &format!("AVAILABLE  {} CAPABILITIES  INSPECT", row.capabilities),
            theme.text_secondary,
        );
        targets.push(HitTarget {
            action: GuiAction::InspectCandidate(row.candidate_id.clone()),
            shape: HitShape::Rect(bounds),
        });
    }
    if view.wants_to_join.is_empty() {
        text(
            target,
            Point::new(left, candidates_y + 26),
            "No current admission requests",
            theme.text_secondary,
        );
    }
    let spawn_y = candidates_y + 54 + view.wants_to_join.len() as i32 * 54;
    if view.actions.contains(&PartsAction::SpawnBrowserPart) {
        let bounds = PixelRect {
            x: left,
            y: spawn_y,
            width: 184,
            height: 30,
        };
        frame_rect(target, bounds, theme.focus, 1);
        text(
            target,
            Point::new(left + 10, spawn_y + 9),
            "+ BROWSER PART",
            theme.emphasis,
        );
        targets.push(HitTarget {
            action: GuiAction::SpawnBrowserPart,
            shape: HitShape::Rect(bounds),
        });
    }
    let details_y = spawn_y + 46;
    if let Some(row) =
        selected.and_then(|part| view.parts.iter().find(|row| &row.details.part_id == part))
    {
        let y = details_y;
        text(target, Point::new(left, y), "EXACT PART", theme.emphasis);
        text(
            target,
            Point::new(left, y + 20),
            row.details.part_id.as_str(),
            theme.text_primary,
        );
        if let Some(host) = &row.details.host_id {
            text(
                target,
                Point::new(left, y + 40),
                &format!("HOST  {}", host.as_str()),
                theme.text_secondary,
            );
        }
        if let Some(boot) = &row.details.boot_id {
            text(
                target,
                Point::new(left, y + 60),
                &format!("BOOT  {}", boot.as_str()),
                theme.text_secondary,
            );
        }
        if let Some(offer) = row.details.offer_generation {
            text(
                target,
                Point::new(left, y + 80),
                &format!("OFFER  {}", offer.0),
                theme.text_secondary,
            );
        }
        if let Some(proof) = &row.details.proof_reference {
            text(
                target,
                Point::new(left, y + 100),
                &format!("PROOF  {proof}"),
                theme.text_secondary,
            );
        }
        if !row.details.planned_placements.is_empty() {
            text(
                target,
                Point::new(left, y + 120),
                &format!(
                    "PLAN  {} PLACEMENTS  {} AUTHORITY  {} EXPECTED SIGNS",
                    row.details.planned_placements.len(),
                    row.details.planned_authority_bindings,
                    row.details.expected_signs
                ),
                theme.text_secondary,
            );
        }
        if row.state != PartPresentationState::Here {
            let bounds = PixelRect {
                x: left,
                y: y + 140,
                width: 184,
                height: 28,
            };
            let confirming = pending_revoke == Some(&row.details.part_id);
            frame_rect(target, bounds, theme.focus, if confirming { 2 } else { 1 });
            text(
                target,
                Point::new(left + 12, y + 148),
                if confirming {
                    "CONFIRM REVOKE"
                } else {
                    "REVOKE PART"
                },
                theme.emphasis,
            );
            targets.push(HitTarget {
                action: if confirming {
                    GuiAction::ConfirmRevokePart(row.details.part_id.clone())
                } else {
                    GuiAction::RequestRevokePart(row.details.part_id.clone())
                },
                shape: HitShape::Rect(bounds),
            });
        }
    } else if let Some(row) = selected_candidate.and_then(|candidate| {
        view.wants_to_join
            .iter()
            .find(|row| &row.candidate_id == candidate)
    }) {
        text(
            target,
            Point::new(left, details_y),
            "EXACT CANDIDATE",
            theme.emphasis,
        );
        text(
            target,
            Point::new(left, details_y + 20),
            row.candidate_id.as_str(),
            theme.text_primary,
        );
        text(
            target,
            Point::new(left, details_y + 40),
            &format!("HOST  {}", row.host_id.as_str()),
            theme.text_secondary,
        );
        text(
            target,
            Point::new(left, details_y + 60),
            &format!("BOOT  {}", row.boot_id.as_str()),
            theme.text_secondary,
        );
        text(
            target,
            Point::new(left, details_y + 80),
            &format!("OFFER  {}  NOT ADMITTED", row.offer_generation.0),
            theme.text_secondary,
        );
        if row.actions.contains(&PartsAction::Refuse) {
            let bounds = PixelRect {
                x: left,
                y: details_y + 96,
                width: 96,
                height: 28,
            };
            frame_rect(target, bounds, theme.focus, 1);
            text(
                target,
                Point::new(left + 12, details_y + 104),
                "REFUSE",
                theme.emphasis,
            );
            targets.push(HitTarget {
                action: GuiAction::RefuseCandidate(row.candidate_id.clone()),
                shape: HitShape::Rect(bounds),
            });
        }
    }
}
