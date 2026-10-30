//! Local Mask presentation choices. None of these actions invokes a Body action,
//! changes a measurement, or changes a producer's freshness or clock basis.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Navy,
    Editorial,
    Engineering,
}
impl Theme {
    pub fn look(self) -> Result<crate::LookInfo, conduit_plot::rust_binding::NativeBindingRefusal> {
        match self {
            Self::Navy => crate::navy_look(
                crate::VisualDensity::Compact,
                crate::VisualTypography::Editorial,
            ),
            Self::Editorial => crate::editorial_look(),
            Self::Engineering => crate::engineering_look(),
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Navy => "Navy",
            Self::Editorial => "Editorial",
            Self::Engineering => "Engineering",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Navy => Self::Editorial,
            Self::Editorial => Self::Engineering,
            Self::Engineering => Self::Navy,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}
impl TemperatureUnit {
    pub const fn unit(self) -> conduit_core::Unit {
        match self {
            Self::Celsius => conduit_core::Unit::Celsius,
            Self::Fahrenheit => conduit_core::Unit::Fahrenheit,
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Celsius => "°C",
            Self::Fahrenheit => "°F",
        }
    }
}
/// Visible time extent relative to the newest retained producer timestamp.
/// This neither requests new observations nor aggregates/interpolates samples.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryWindow {
    #[default]
    Retained,
    Hour,
    QuarterHour,
    FiveMinutes,
    Minute,
}
impl HistoryWindow {
    pub const fn milliseconds(self) -> Option<u64> {
        match self {
            Self::Retained => None,
            Self::Hour => Some(3_600_000),
            Self::QuarterHour => Some(900_000),
            Self::FiveMinutes => Some(300_000),
            Self::Minute => Some(60_000),
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Retained => "All retained",
            Self::Hour => "1 hour",
            Self::QuarterHour => "15 min",
            Self::FiveMinutes => "5 min",
            Self::Minute => "1 min",
        }
    }
    fn zoom(self, inward: bool) -> Self {
        use HistoryWindow::*;
        match (self, inward) {
            (Retained, true) | (QuarterHour, false) => Hour,
            (Hour, true) | (FiveMinutes, false) => QuarterHour,
            (QuarterHour, true) | (Minute, false) => FiveMinutes,
            (FiveMinutes | Minute, true) => Minute,
            _ => Retained,
        }
    }
}
/// Divider location is local normalized presentation geometry, never Face identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitLayout {
    Split { left_per_mille: u16 },
    CollapsedLeft,
    CollapsedRight,
}
impl Default for SplitLayout {
    fn default() -> Self {
        Self::Split {
            left_per_mille: 360,
        }
    }
}
impl SplitLayout {
    pub const fn from_divider(per_mille: u16) -> Self {
        if per_mille <= 60 {
            Self::CollapsedLeft
        } else if per_mille >= 940 {
            Self::CollapsedRight
        } else {
            Self::Split {
                left_per_mille: per_mille,
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationPreferences {
    pub theme: Theme,
    #[serde(default)]
    pub details: bool,
    #[serde(default)]
    pub split: SplitLayout,
    pub temperature: TemperatureUnit,
    pub history_window: HistoryWindow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationAction {
    MoveDivider(u16),
    NextTheme,
    ToggleDetails,
    ToggleTemperatureUnit,
    ZoomIn,
    ZoomOut,
    ResetHistoryWindow,
}
impl PresentationPreferences {
    pub fn apply(&mut self, action: PresentationAction) -> bool {
        let previous = *self;
        match action {
            PresentationAction::MoveDivider(position) => {
                self.split = SplitLayout::from_divider(position)
            }
            PresentationAction::ToggleDetails => self.details = !self.details,
            PresentationAction::NextTheme => self.theme = self.theme.next(),
            PresentationAction::ToggleTemperatureUnit => {
                self.temperature = match self.temperature {
                    TemperatureUnit::Celsius => TemperatureUnit::Fahrenheit,
                    TemperatureUnit::Fahrenheit => TemperatureUnit::Celsius,
                }
            }
            PresentationAction::ZoomIn => self.history_window = self.history_window.zoom(true),
            PresentationAction::ZoomOut => self.history_window = self.history_window.zoom(false),
            PresentationAction::ResetHistoryWindow => self.history_window = HistoryWindow::Retained,
        }
        previous != *self
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn details_is_local_and_old_saved_preferences_remain_valid() {
        let old = r#"{"theme":"Navy","temperature":"Celsius","history_window":"Retained"}"#;
        let mut preferences: PresentationPreferences = serde_json::from_str(old).unwrap();
        assert!(!preferences.details);
        let original = preferences;
        assert!(preferences.apply(PresentationAction::ToggleDetails));
        assert!(preferences.details);
        assert!(preferences.apply(PresentationAction::ToggleDetails));
        assert_eq!(preferences, original);
    }
    #[test]
    fn zoom_is_bounded_and_reversible_and_does_not_change_other_preferences() {
        let mut p = PresentationPreferences::default();
        for _ in 0..20 {
            p.apply(PresentationAction::ZoomIn);
        }
        assert_eq!(p.history_window, HistoryWindow::Minute);
        assert!(!p.apply(PresentationAction::ZoomIn));
        assert_eq!(p.theme, Theme::Navy);
        assert_eq!(p.temperature, TemperatureUnit::Celsius);
        for _ in 0..20 {
            p.apply(PresentationAction::ZoomOut);
        }
        assert_eq!(p, PresentationPreferences::default());
    }
    #[test]
    fn selected_themes_are_existing_valid_looks_and_units_remain_dimensions() {
        for theme in [Theme::Navy, Theme::Editorial, Theme::Engineering] {
            assert!(theme.look().is_ok());
        }
        let mut p = PresentationPreferences::default();
        p.apply(PresentationAction::ToggleTemperatureUnit);
        assert_eq!(p.temperature.unit(), conduit_core::Unit::Fahrenheit);
        assert_eq!(p.history_window, HistoryWindow::Retained);
    }
}
