//! Configured collection column admission over existing measured row/panel layout.
use super::*;

/// Recipe group identity is not a domain catalogue or a reading-order hint.
/// The renderer's first pair follows authored region order, even when the
/// complete primary reader encounters review/unmatched material first.
pub(in crate::native_face_scene) fn authored_panel_groups(
    groups: impl Iterator<Item = u8>,
) -> Option<[u8; 2]> {
    let mut first = None;
    for group in groups {
        match first {
            None => first = Some(group),
            Some(prior) if prior != group => return Some([prior, group]),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
pub(in crate::native_face_scene) fn admit_configured(
    items: Vec<document::Item>,
    screen: LayoutRect,
    gutter: u16,
    columns: u16,
    text_scale_percent: u16,
    padding: u16,
    groups: Option<[u8; 2]>,
) -> Result<Vec<Row>, FaceSceneError> {
    admit_configured_selected(
        items,
        screen,
        ConfiguredColumns {
            gutter,
            columns,
            spacing: RowSpacing::legacy(padding),
            groups,
        },
        text_scale_percent,
        Typography::EDITORIAL,
    )
}

/// Recipe geometry and density-derived spacing for configured collection flow.
pub(in crate::native_face_scene) struct ConfiguredColumns {
    pub gutter: u16,
    pub columns: u16,
    pub spacing: RowSpacing,
    pub groups: Option<[u8; 2]>,
}

pub(in crate::native_face_scene) fn admit_configured_selected(
    items: Vec<document::Item>,
    screen: LayoutRect,
    geometry: ConfiguredColumns,
    text_scale_percent: u16,
    typography: Typography,
) -> Result<Vec<Row>, FaceSceneError> {
    let ConfiguredColumns {
        gutter,
        columns,
        spacing,
        groups,
    } = geometry;
    let spacing = spacing.validate()?;
    if !(1..=2).contains(&columns) {
        return Err(FaceSceneError::InvalidExtent);
    }
    let pair = if columns == 2 && screen.width >= 640 {
        groups
    } else {
        None
    };
    if pair.is_some() || (spacing.panel.is_some() && items.iter().any(|item| item.region.is_some()))
    {
        if pair.is_some_and(|groups| groups[0] == groups[1]) {
            return Err(FaceSceneError::InvalidExtent);
        }
        panels::admit(
            items,
            screen,
            gutter,
            text_scale_percent,
            spacing,
            pair,
            typography,
        )
    } else {
        admit_with_expression_selected(
            items,
            screen,
            gutter,
            columns,
            text_scale_percent,
            spacing,
            typography,
        )
    }
}
