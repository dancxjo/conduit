use conduit_home_model::HomeView;

pub struct NativeHomeLayout;

impl NativeHomeLayout {
    pub fn hit_index(view: HomeView, width: u32, x: f64, y: f64) -> Option<usize> {
        if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
            return None;
        }
        match view {
            HomeView::Launcher => launcher_hit(width as usize, x as usize, y as usize),
            HomeView::Forms => forms_hit(width as usize, x as usize, y as usize),
            _ => None,
        }
    }
}

fn launcher_hit(width: usize, x: usize, y: usize) -> Option<usize> {
    let available_width = width.saturating_sub(80);
    let card_width = available_width.saturating_sub(24) / 3;
    if card_width == 0 || x < 28 || y < 122 {
        return None;
    }
    let stride_x = card_width + 12;
    let stride_y = 146;
    let column = (x - 28) / stride_x;
    let row = (y - 122) / stride_y;
    if column >= 3 || row >= 2 {
        return None;
    }
    let local_x = (x - 28) % stride_x;
    let local_y = (y - 122) % stride_y;
    (local_x < card_width && local_y < 130).then_some(row * 3 + column)
}

fn forms_hit(width: usize, x: usize, y: usize) -> Option<usize> {
    if x < 28 || x >= width.saturating_sub(28) || y < 164 {
        return None;
    }
    let row = (y - 164) / 42;
    (row < crate::INSTALLED_FORMS.len()).then_some(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_cards_are_clickable_and_gaps_are_not() {
        assert_eq!(
            NativeHomeLayout::hit_index(HomeView::Launcher, 1120, 40.0, 140.0),
            Some(0)
        );
        assert_eq!(
            NativeHomeLayout::hit_index(HomeView::Launcher, 1120, 750.0, 290.0),
            Some(5)
        );
        assert_eq!(
            NativeHomeLayout::hit_index(HomeView::Launcher, 1120, 370.0, 140.0),
            None
        );
    }

    #[test]
    fn forms_map_only_visible_action_rows() {
        assert_eq!(
            NativeHomeLayout::hit_index(HomeView::Forms, 1120, 50.0, 170.0),
            Some(0)
        );
        assert_eq!(
            NativeHomeLayout::hit_index(HomeView::Forms, 1120, 50.0, 300.0),
            Some(3)
        );
    }
}
