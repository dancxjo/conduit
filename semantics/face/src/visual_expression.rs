//! Finite visual expression, independent of semantic slots and component layout.
//!
//! This is the first visual admission seam, not the common multimodal Look
//! declaration language. Canonical Look admission owns retired-wire refusal.
use alloc::string::{String, ToString};
use serde::{Deserialize, Serialize};

pub const VISUAL_EXPRESSION_VERSION: u16 = 2;
pub const MAX_VISUAL_EXPRESSION_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VisualDensity {
    Compact,
    Spacious,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VisualPalette {
    Phosphor,
    Editorial,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VisualMotion {
    None,
    Restrained,
}

/// Finite typographic character; exact fonts remain admitted Host resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VisualTypography {
    Editorial,
    Engineering,
}

/// Closed expression fields: no selector, action, ordering or input authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualExpression {
    pub version: u16,
    pub identity: String,
    pub density: VisualDensity,
    pub palette: VisualPalette,
    pub text_scale_percent: u16,
    pub motion: VisualMotion,
    pub typography: VisualTypography,
}
impl VisualExpression {
    pub fn phosphor() -> Self {
        Self {
            version: VISUAL_EXPRESSION_VERSION,
            identity: crate::PHOSPHOR_LOOK_ID.to_string(),
            density: VisualDensity::Compact,
            palette: VisualPalette::Phosphor,
            text_scale_percent: 100,
            motion: VisualMotion::None,
            typography: VisualTypography::Editorial,
        }
    }
    pub fn validate(&self) -> Result<(), VisualExpressionRefusal> {
        if self.version != VISUAL_EXPRESSION_VERSION {
            return Err(VisualExpressionRefusal::Version);
        }
        if self.identity.is_empty()
            || self.identity.len() > MAX_VISUAL_EXPRESSION_ID_BYTES
            || matches!(
                self.identity.as_str(),
                "conduit.face/phosphor@1" | "conduit.presentation/phosphor@1"
            )
        {
            return Err(VisualExpressionRefusal::Identity);
        }
        if !(100..=200).contains(&self.text_scale_percent) {
            return Err(VisualExpressionRefusal::TextScale);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualExpressionRefusal {
    Encoding,
    Version,
    Identity,
    TextScale,
    HostTextCapacity,
}

/// Observed user requirements and selected Host limits, never Look preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualUserConstraints {
    pub forced_colors: bool,
    pub reduced_motion: bool,
    pub minimum_text_scale_percent: u16,
    pub maximum_supported_text_scale_percent: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum VisualExpressionOutcome {
    Selected,
    AccessibilityOverride {
        forced_colors: bool,
        reduced_motion: bool,
        text_scale: bool,
    },
    PhosphorFallback {
        refusal: VisualExpressionRefusal,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct VisualAccessibilityOverrides {
    pub forced_colors: bool,
    pub reduced_motion: bool,
    pub text_scale: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AdmittedVisualExpression {
    pub expression: VisualExpression,
    pub outcome: VisualExpressionOutcome,
    /// Applied requirements remain observable even when the selected Look fails.
    pub accessibility_overrides: VisualAccessibilityOverrides,
    /// The Host must use platform forced colors instead of these paint tokens.
    pub platform_forced_colors: bool,
}

/// Invalid expression keeps a usable Phosphor request; impossible actual Host
/// text capacity remains a distinct refusal for the Mask's accessible fallback.
pub fn admit_visual_expression(
    requested: Result<VisualExpression, VisualExpressionRefusal>,
    user: VisualUserConstraints,
) -> Result<AdmittedVisualExpression, VisualExpressionRefusal> {
    let requested = requested.and_then(|value| {
        value.validate()?;
        Ok(value)
    });
    let (mut expression, mut outcome) = match requested {
        Ok(value) => (value, VisualExpressionOutcome::Selected),
        Err(refusal) => (
            VisualExpression::phosphor(),
            VisualExpressionOutcome::PhosphorFallback { refusal },
        ),
    };
    if !(100..=200).contains(&user.minimum_text_scale_percent)
        || user.minimum_text_scale_percent > user.maximum_supported_text_scale_percent
    {
        return Err(VisualExpressionRefusal::HostTextCapacity);
    }
    if expression.text_scale_percent > user.maximum_supported_text_scale_percent {
        expression = VisualExpression::phosphor();
        outcome = VisualExpressionOutcome::PhosphorFallback {
            refusal: VisualExpressionRefusal::HostTextCapacity,
        };
    }
    let required_scale = expression
        .text_scale_percent
        .max(user.minimum_text_scale_percent);
    let text_scale = required_scale != expression.text_scale_percent;
    expression.text_scale_percent = required_scale;
    let reduced_motion = user.reduced_motion && expression.motion != VisualMotion::None;
    if user.reduced_motion {
        expression.motion = VisualMotion::None;
    }
    if matches!(outcome, VisualExpressionOutcome::Selected)
        && (user.forced_colors || reduced_motion || text_scale)
    {
        outcome = VisualExpressionOutcome::AccessibilityOverride {
            forced_colors: user.forced_colors,
            reduced_motion,
            text_scale,
        };
    }
    Ok(AdmittedVisualExpression {
        expression,
        outcome,
        accessibility_overrides: VisualAccessibilityOverrides {
            forced_colors: user.forced_colors,
            reduced_motion: user.reduced_motion,
            text_scale,
        },
        platform_forced_colors: user.forced_colors,
    })
}

/// Strict finite wire decoding is separate from selected-Host admission.
pub fn decode_visual_expression(bytes: &[u8]) -> Result<VisualExpression, VisualExpressionRefusal> {
    if bytes.len() > 1024 {
        return Err(VisualExpressionRefusal::Encoding);
    }
    let value: VisualExpression =
        serde_json::from_slice(bytes).map_err(|_| VisualExpressionRefusal::Encoding)?;
    value.validate()?;
    Ok(value)
}
