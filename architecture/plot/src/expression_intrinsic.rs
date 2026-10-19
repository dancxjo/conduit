//! Reviewed pure call recognition shared by Source and portable programs.
pub(crate) fn is_intrinsic(name: &str) -> bool {
    crate::expression_inverse_trigonometry::function(name).is_some()
        || crate::expression_trigonometry::function(name).is_some()
        || name == crate::expression_quantity_value::CALL
        || name == crate::expression_square_root::CALL
        || crate::expression_angle_reference::is_call(name)
        || crate::expression_semantic_call::integer_widening_target(name).is_some()
        || matches!(
            name,
            "text/material"
                | "variant/tag"
                | "variant/is"
                | "sequence/length"
                | "sequence/at"
                | "bytes/length"
                | "bytes/at"
                | "bytes/window"
        )
}
