//! Finite renderer-local state for the authoritative Gear chooser.

use patchbay_model::{GearPalette, PaletteError, MAX_PALETTE_QUERY_BYTES};

pub(super) const MAX_VISIBLE_PALETTE_RESULTS: usize = 3;
pub(super) const MAX_VISIBLE_PLACEMENT_SLOTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PaletteMove {
    Previous,
    Next,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PaletteChooserError {
    ResultBoundExceeded,
    CatalogUnavailable,
    QueryBoundExceeded,
    NoResults,
    ScrollBoundReached,
    PlacementSlotsExhausted,
    PlacementCoordinateOutOfBounds,
}

impl PaletteChooserError {
    pub(super) const fn message(&self) -> &'static str {
        match self {
            Self::ResultBoundExceeded => {
                "Palette results refused: the authoritative catalog exceeds its finite bound"
            }
            Self::CatalogUnavailable => {
                "Palette results refused: authoritative catalog metadata is unavailable"
            }
            Self::QueryBoundExceeded => {
                "Palette query refused: the finite query byte bound was reached"
            }
            Self::NoResults => "Palette placement refused: the query has no result",
            Self::ScrollBoundReached => {
                "Palette scroll refused: selection is at the finite result boundary"
            }
            Self::PlacementSlotsExhausted => {
                "Palette placement refused: no bounded visible keyboard target remains"
            }
            Self::PlacementCoordinateOutOfBounds => {
                "Palette placement refused: the pointer target is outside finite canvas bounds"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct PaletteChooser {
    query: String,
    search_active: bool,
    selected_result: usize,
    scroll_offset: usize,
}

impl PaletteChooser {
    #[cfg(test)]
    pub(super) fn for_query(query: &str) -> Self {
        let mut chooser = Self::default();
        chooser.focus();
        chooser.append(query).expect("bounded test palette query");
        chooser
    }

    pub(super) fn query(&self) -> &str {
        &self.query
    }

    pub(super) const fn search_active(&self) -> bool {
        self.search_active
    }

    pub(super) const fn selected_result(&self) -> usize {
        self.selected_result
    }

    pub(super) const fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub(super) fn focus(&mut self) {
        self.search_active = true;
        self.normalize_selection();
    }

    pub(super) fn append(&mut self, value: &str) -> Result<(), PaletteChooserError> {
        let next_len = self
            .query
            .len()
            .checked_add(value.len())
            .ok_or(PaletteChooserError::QueryBoundExceeded)?;
        if next_len > MAX_PALETTE_QUERY_BYTES {
            return Err(PaletteChooserError::QueryBoundExceeded);
        }
        self.query.push_str(value);
        self.reset_selection();
        Ok(())
    }

    pub(super) fn backspace(&mut self) {
        self.query.pop();
        self.reset_selection();
    }

    pub(super) fn exit_search(&mut self) {
        self.query.clear();
        self.search_active = false;
        self.reset_selection();
    }

    pub(super) fn move_selection(
        &mut self,
        direction: PaletteMove,
    ) -> Result<(), PaletteChooserError> {
        let count = self.result_count()?;
        if count == 0 {
            return Err(PaletteChooserError::NoResults);
        }
        let next = match direction {
            PaletteMove::Previous => self.selected_result.checked_sub(1),
            PaletteMove::Next => self
                .selected_result
                .checked_add(1)
                .filter(|candidate| *candidate < count),
        }
        .ok_or(PaletteChooserError::ScrollBoundReached)?;
        self.selected_result = next;
        self.scroll_offset = self.scroll_offset.min(self.selected_result).max(
            self.selected_result
                .saturating_add(1)
                .saturating_sub(MAX_VISIBLE_PALETTE_RESULTS),
        );
        Ok(())
    }

    pub(super) fn select_kind(&mut self, kind: &str) -> Result<(), PaletteChooserError> {
        let palette = palette()?;
        let results = palette.search(&self.query).map_err(map_palette_error)?;
        let index = results
            .iter()
            .position(|entry| entry.kind_id.as_str() == kind)
            .ok_or(PaletteChooserError::NoResults)?;
        self.selected_result = index;
        self.scroll_offset = index.saturating_sub(MAX_VISIBLE_PALETTE_RESULTS - 1);
        Ok(())
    }

    pub(super) fn selected_kind(&self) -> Result<String, PaletteChooserError> {
        let palette = palette()?;
        palette
            .search(&self.query)
            .map_err(map_palette_error)?
            .get(self.selected_result)
            .map(|entry| entry.kind_id.as_str().to_owned())
            .ok_or(PaletteChooserError::NoResults)
    }

    pub(super) fn result_count(&self) -> Result<usize, PaletteChooserError> {
        let palette = palette()?;
        palette
            .search(&self.query)
            .map(|results| results.len())
            .map_err(map_palette_error)
    }

    pub(super) fn keyboard_target(
        visible_subject_count: usize,
    ) -> Result<(i32, i32), PaletteChooserError> {
        if visible_subject_count >= MAX_VISIBLE_PLACEMENT_SLOTS {
            return Err(PaletteChooserError::PlacementSlotsExhausted);
        }
        let column = visible_subject_count % 2;
        let row = visible_subject_count / 2;
        Ok((204 + column as i32 * 254, 80 + row as i32 * 150))
    }

    pub(super) fn pointer_target(x: f64, y: f64) -> Result<(i32, i32), PaletteChooserError> {
        if !x.is_finite()
            || !y.is_finite()
            || x <= 176.0
            || x > f64::from(patchbay_model::MAX_LAYOUT_COORDINATE) + 95.0
            || y < 53.0
            || y > f64::from(patchbay_model::MAX_LAYOUT_COORDINATE) + 20.0
        {
            return Err(PaletteChooserError::PlacementCoordinateOutOfBounds);
        }
        Ok(((x as i32 - 95).max(177), (y as i32 - 20).max(53)))
    }

    fn reset_selection(&mut self) {
        self.selected_result = 0;
        self.scroll_offset = 0;
    }

    fn normalize_selection(&mut self) {
        let count = self.result_count().unwrap_or(0);
        if count == 0 {
            self.reset_selection();
        } else if self.selected_result >= count {
            self.selected_result = count - 1;
            self.scroll_offset = self
                .selected_result
                .saturating_sub(MAX_VISIBLE_PALETTE_RESULTS - 1);
        }
    }
}

fn palette() -> Result<GearPalette, PaletteChooserError> {
    GearPalette::standard().map_err(map_palette_error)
}

fn map_palette_error(error: PaletteError) -> PaletteChooserError {
    match error {
        PaletteError::CatalogTooLarge => PaletteChooserError::ResultBoundExceeded,
        PaletteError::QueryTooLarge => PaletteChooserError::QueryBoundExceeded,
        PaletteError::MissingMetadata(_) => PaletteChooserError::CatalogUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooser_bounds_query_results_scroll_and_placement_independently() {
        let mut chooser = PaletteChooser::default();
        chooser.focus();
        assert!(chooser.search_active());
        assert_eq!(
            chooser.append(&"x".repeat(MAX_PALETTE_QUERY_BYTES + 1)),
            Err(PaletteChooserError::QueryBoundExceeded)
        );
        assert_eq!(
            chooser.move_selection(PaletteMove::Previous),
            Err(PaletteChooserError::ScrollBoundReached)
        );
        assert_eq!(
            PaletteChooser::keyboard_target(MAX_VISIBLE_PLACEMENT_SLOTS),
            Err(PaletteChooserError::PlacementSlotsExhausted)
        );
        assert_eq!(
            PaletteChooser::pointer_target(f64::INFINITY, 80.0),
            Err(PaletteChooserError::PlacementCoordinateOutOfBounds)
        );
    }

    #[test]
    fn uppercase_is_a_bounded_authoritative_keyboard_result() {
        let mut chooser = PaletteChooser::default();
        chooser.focus();
        chooser.append("uppercase").unwrap();
        assert_eq!(chooser.result_count().unwrap(), 1);
        assert_eq!(chooser.selected_kind().unwrap(), "text/upper");
        assert_eq!(PaletteChooser::keyboard_target(0).unwrap(), (204, 80));
        chooser.exit_search();
        assert!(!chooser.search_active());
        assert!(chooser.query().is_empty());
    }

    #[test]
    fn finite_selection_scrolls_without_wrapping_and_empty_query_is_distinct() {
        let mut chooser = PaletteChooser::default();
        chooser.focus();
        chooser.move_selection(PaletteMove::Next).unwrap();
        assert_eq!(chooser.selected_result(), 1);
        for _ in 0..MAX_VISIBLE_PALETTE_RESULTS {
            chooser.move_selection(PaletteMove::Next).unwrap();
        }
        assert!(chooser.scroll_offset() > 0);
        chooser.append("no-such-authoritative-kind").unwrap();
        assert_eq!(chooser.result_count().unwrap(), 0);
        assert_eq!(chooser.selected_kind(), Err(PaletteChooserError::NoResults));
    }
}
