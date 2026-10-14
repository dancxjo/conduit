//! The visual facet's actual ordinary structured startup contract.
use crate::{
    VisualDensity, VisualDensityPreference, VisualExpression, VisualExpressionInfo,
    VisualExpressionRefusal, VisualMotion, VisualMotionPreference, VisualPalette,
    VisualPalettePreference,
};
use conduit_core::{StructuredInfoType, StructuredInfoValue};
use conduit_plot::rust_binding::NativeRustBinding;

pub fn visual_expression_type() -> StructuredInfoType {
    VisualExpressionInfo::semantic_type().expect("checked finite visual expression Type")
}

pub fn visual_expression_info(
    expression: &VisualExpression,
) -> Result<StructuredInfoValue, VisualExpressionRefusal> {
    expression.validate()?;
    VisualExpressionInfo::new(
        match expression.density {
            VisualDensity::Compact => VisualDensityPreference::Compact,
            VisualDensity::Spacious => VisualDensityPreference::Spacious,
        },
        expression.identity.clone(),
        match expression.motion {
            VisualMotion::None => VisualMotionPreference::None,
            VisualMotion::Restrained => VisualMotionPreference::Restrained,
        },
        match expression.palette {
            VisualPalette::Phosphor => VisualPalettePreference::Phosphor,
            VisualPalette::Editorial => VisualPalettePreference::Editorial,
        },
        expression.text_scale_percent,
        expression.version,
    )
    .map_err(|_| VisualExpressionRefusal::Encoding)?
    .into_structured()
    .map_err(|_| VisualExpressionRefusal::Encoding)
}

/// Decode exact nominal/record schema and leaf contracts before visual admission.
/// Arbitrary structured bytes with a copied profile are insufficient.
pub fn visual_expression_from_info(
    value: StructuredInfoValue,
) -> Result<VisualExpression, VisualExpressionRefusal> {
    let value = VisualExpressionInfo::from_structured(value)
        .map_err(|_| VisualExpressionRefusal::Encoding)?;
    let expression = VisualExpression {
        version: *value.version(),
        identity: value.identity().clone(),
        density: match value.density() {
            VisualDensityPreference::Compact => VisualDensity::Compact,
            VisualDensityPreference::Spacious => VisualDensity::Spacious,
        },
        palette: match value.palette() {
            VisualPalettePreference::Phosphor => VisualPalette::Phosphor,
            VisualPalettePreference::Editorial => VisualPalette::Editorial,
        },
        motion: match value.motion() {
            VisualMotionPreference::None => VisualMotion::None,
            VisualMotionPreference::Restrained => VisualMotion::Restrained,
        },
        text_scale_percent: *value.text_scale_percent(),
    };
    expression.validate()?;
    Ok(expression)
}
