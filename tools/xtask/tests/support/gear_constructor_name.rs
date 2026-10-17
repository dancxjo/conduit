//! Recognize a constructor name without mistaking a bare let-pattern for one.
pub fn has_constructor_name(line: &str, type_name: &str) -> bool {
    let needle = format!("{type_name} {{");
    line.match_indices(&needle).any(|(offset, _)| {
        let exact = offset == 0
            || !line.as_bytes()[offset - 1].is_ascii_alphanumeric()
                && line.as_bytes()[offset - 1] != b'_';
        // A name immediately after `let` begins a pattern, not a value. Check
        // each occurrence independently: its initializer may still construct
        // the same type on this very line and must remain subject to the guard.
        exact && line[..offset].trim() != "let"
    })
}
#[test]
fn exhaustive_patterns_do_not_create_checked_or_planned_gears() {
    for kind in ["CheckedGear", "PlannedGear"] {
        assert!(!has_constructor_name(&format!("    let {kind} {{"), kind));
        assert!(has_constructor_name(
            &format!("    let value = {kind} {{"),
            kind
        ));
        assert!(has_constructor_name(&format!("    crate::{kind} {{"), kind));
        assert!(has_constructor_name(
            &format!("    let {kind} {{ gear_id, .. }} = {kind} {{"),
            kind
        ));
        assert!(has_constructor_name(
            &format!("    let {kind} {{ gear_id, .. }} = crate::{kind} {{"),
            kind
        ));
        assert!(!has_constructor_name(&format!("Other{kind} {{"), kind));
        assert!(!has_constructor_name(&format!("Other_{kind} {{"), kind));
    }
}
