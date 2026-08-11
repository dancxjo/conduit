//! Direct presenter interpretation of portable composition obligations.
//!
//! The direct path keeps an eager presenter-owned list. Normalization alone
//! crosses into the portable fixed encoding; evaluation does not call the
//! reference composition operations.

use conduit_presentation::{
    AccessibilityRole, CompositionError, CompositionItem, CompositionItemKind, LayoutAlignment,
    LayoutFrame, PresentationComposition,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectObligation {
    pub kind: CompositionItemKind,
    pub role: AccessibilityRole,
    pub token: String,
    pub accessibility_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectPresentation {
    obligations: Vec<DirectObligation>,
}

impl DirectPresentation {
    pub fn icon(token: &str, accessibility_name: &str) -> Result<Self, CompositionError> {
        if !conduit_presentation::is_authoritative_icon(token) {
            return Err(CompositionError::UnknownIcon);
        }
        Ok(Self {
            obligations: vec![DirectObligation {
                kind: CompositionItemKind::Icon,
                role: AccessibilityRole::Image,
                token: token.into(),
                accessibility_name: accessibility_name.into(),
            }],
        })
    }

    pub fn frame(mut self, role: &str, accessibility_name: &str) -> Self {
        self.obligations.push(DirectObligation {
            kind: CompositionItemKind::Frame,
            role: AccessibilityRole::Group,
            token: role.into(),
            accessibility_name: accessibility_name.into(),
        });
        self
    }

    pub fn badge(mut self, state: &str, accessibility_name: &str) -> Self {
        self.obligations.push(DirectObligation {
            kind: CompositionItemKind::Badge,
            role: AccessibilityRole::Status,
            token: state.into(),
            accessibility_name: accessibility_name.into(),
        });
        self
    }

    pub fn normalize(&self) -> Result<PresentationComposition, CompositionError> {
        let mut output = PresentationComposition::empty();
        for obligation in &self.obligations {
            output.push(CompositionItem::new(
                obligation.kind,
                obligation.role,
                &obligation.token,
                &obligation.accessibility_name,
            )?)?;
        }
        Ok(output)
    }
}

/// Honest constrained-presenter preparation: semantic obligations remain
/// separate while #888 supplies finite region placement. #890 owns converting
/// those resolved leaves into accepted graphics operations.
pub fn constrained_frame_layout(
    width: u16,
    height: u16,
) -> Result<LayoutFrame, conduit_presentation::LayoutError> {
    LayoutFrame::viewport(width, height, 2, 24, 12)?
        .inset(4)?
        .align(LayoutAlignment::Center, LayoutAlignment::Center)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_and_reference_presenters_normalize_the_same_obligations() {
        let reference = PresentationComposition::icon("presentation", "Patchbay")
            .unwrap()
            .frame("panel", "Gear Face")
            .unwrap()
            .badge("warning", "Cord pressure")
            .unwrap();
        let direct = DirectPresentation::icon("presentation", "Patchbay")
            .unwrap()
            .frame("panel", "Gear Face")
            .badge("warning", "Cord pressure")
            .normalize()
            .unwrap();
        assert_eq!(reference, direct);
        assert_eq!(reference.encode(), direct.encode());

        let layout = constrained_frame_layout(120, 80).unwrap();
        assert_eq!(layout.child_count, 2);
        assert_eq!(layout.viewport.x, 4);
        assert_eq!(layout.children[0].x, 50);
    }
}
