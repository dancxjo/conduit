//! Categorized native presentation of the authoritative Gear palette.

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{PaletteCategory, PatchbayTheme};

use crate::{
    gui::{GuiAction, HitTarget},
    gui_hit::HitShape,
    gui_primitives::{frame_rect, rgb, text, PixelRect},
    palette_icon::draw_palette_icon,
};

pub(super) fn draw_palette<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    chooser: &crate::palette_state::PaletteChooser,
    placement_target: Result<(i32, i32), crate::palette_state::PaletteChooserError>,
    mut y: i32,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let Ok(palette) = patchbay_model::GearPalette::standard() else {
        return;
    };
    let entries = palette.search(chooser.query()).unwrap_or_default();
    if entries.is_empty() {
        text(
            target,
            Point::new(14, y),
            "NO MATCHING GEARS",
            theme.failure,
        );
        text(
            target,
            Point::new(14, y + 17),
            "EDIT QUERY OR ESCAPE",
            theme.text_secondary,
        );
        return;
    }
    text(
        target,
        Point::new(14, y),
        &format!(
            "RESULT {} OF {}  UP/DOWN",
            chooser.selected_result() + 1,
            entries.len()
        ),
        theme.text_secondary,
    );
    y += 17;
    let target_text = match placement_target {
        Ok((x, y)) => format!("ENTER TARGET  {x}, {y}"),
        Err(error) => error.message().to_owned(),
    };
    text(
        target,
        Point::new(14, y),
        &target_text,
        theme.text_secondary,
    );
    y += 17;
    let mut category: Option<PaletteCategory> = None;
    for (index, entry) in entries
        .into_iter()
        .enumerate()
        .skip(chooser.scroll_offset())
        .take(crate::palette_state::MAX_VISIBLE_PALETTE_RESULTS)
    {
        if category != Some(entry.category) {
            category = Some(entry.category);
            text(
                target,
                Point::new(14, y),
                entry.category.label(),
                theme.text_secondary,
            );
            y += 17;
        }
        let entry_height = if index == chooser.selected_result() {
            124
        } else {
            73
        };
        let bounds = PixelRect {
            x: 12,
            y,
            width: 150,
            height: entry_height,
        };
        let selected = index == chooser.selected_result();
        frame_rect(
            target,
            bounds,
            if selected {
                theme.focus
            } else {
                theme.structure_secondary
            },
            if selected { 2 } else { 1 },
        );
        let fallback = draw_palette_icon(
            target,
            entry.icon,
            Point::new(17, y + 3),
            rgb(theme.emphasis),
        );
        text(
            target,
            Point::new(38, y + 4),
            &format!("{}{}", entry.plain_name, if fallback { " !" } else { "" }),
            theme.text_primary,
        );
        if selected {
            text(
                target,
                Point::new(38, y + 71),
                &format!("KIND {}", entry.kind_id.as_str()),
                theme.text_secondary,
            );
            text(
                target,
                Point::new(38, y + 88),
                &exact_contract_line(entry),
                theme.text_secondary,
            );
            text(
                target,
                Point::new(38, y + 105),
                &exact_bounds_line(entry),
                theme.text_secondary,
            );
        }
        let summary = catalog_summary_lines(&entry.summary);
        text(
            target,
            Point::new(38, y + 20),
            &summary[0],
            theme.text_secondary,
        );
        text(
            target,
            Point::new(38, y + 37),
            &summary[1],
            theme.text_secondary,
        );
        text(
            target,
            Point::new(38, y + 54),
            &format!(
                "IN {}  OUT {}  ENTER ADD",
                entry.inputs.len(),
                entry.outputs.len()
            ),
            if selected {
                theme.focus
            } else {
                theme.emphasis
            },
        );
        targets.push(HitTarget {
            action: GuiAction::BeginPaletteDrag(entry.kind_id.as_str().into()),
            shape: HitShape::Rect(bounds),
        });
        y += entry_height as i32 + 1;
    }
}

fn exact_contract_line(entry: &patchbay_model::PaletteEntry) -> String {
    let input = entry
        .inputs
        .first()
        .map(|port| format!("{}:{}", port.port_id.as_str(), port.value_kind.as_str()))
        .unwrap_or_else(|| "none".into());
    let output = entry
        .outputs
        .first()
        .map(|port| format!("{}:{}", port.port_id.as_str(), port.value_kind.as_str()))
        .unwrap_or_else(|| "none".into());
    format!("I {input} O {output}")
}

fn exact_bounds_line(entry: &patchbay_model::PaletteEntry) -> String {
    let configuration = entry
        .configuration
        .first()
        .map(|field| format!("{}:{:?}", field.key, field.rule))
        .unwrap_or_else(|| "none".into());
    format!(
        "CONFIG {configuration} LIMIT A{} Q{}/{}B",
        entry.limits.max_active_instances,
        entry.limits.max_queue_items,
        entry.limits.max_queue_bytes
    )
}

fn catalog_summary_lines(summary: &str) -> [String; 2] {
    const LINE_CHARS: usize = 18;
    let mut words = summary.split_whitespace().peekable();
    let mut lines = [String::new(), String::new()];
    for line in &mut lines {
        while let Some(word) = words.peek().copied() {
            let separator = usize::from(!line.is_empty());
            if line.chars().count() + separator + word.chars().count() > LINE_CHARS {
                break;
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            words.next();
        }
    }
    if words.peek().is_some() {
        let last = &mut lines[1];
        while last.chars().count() > LINE_CHARS.saturating_sub(3) {
            last.pop();
        }
        last.push_str("...");
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_selected_detail_is_derived_from_catalog_contract() {
        let palette = patchbay_model::GearPalette::standard().unwrap();
        let upper = palette
            .find(&conduit_core::KindId::from("text/upper"))
            .unwrap();
        let detail = exact_contract_line(upper);
        assert!(detail.contains("text:value/text@1"));
        let bounds = exact_bounds_line(upper);
        assert!(bounds.contains(&format!(
            "Q{}/{}B",
            upper.limits.max_queue_items, upper.limits.max_queue_bytes
        )));
        let summary = catalog_summary_lines(&upper.summary);
        assert!(summary.join(" ").starts_with("Uppercase one bounded"));
    }
}
