//! Capacity evidence is separate from semantic recovery and physical scanout.
use super::ConduitosError;
use serde_json::Value;

const PREFIX: &str = "CONDUIT_COMPOSITOR_PRESSURE_PROOF ";

fn expected(raster_pixels: u64, guest_entries: u64, recompose_pixels: u64) -> Value {
    serde_json::json!({
        "schema":"conduit.conduitos/compositor-pressure-fixture@1",
        "status":"completed","proof_class":"freestanding-emulator","fixture":true,
        "scene":{
            "commands":48,"text_bytes_per_command":192,
            "command49_refusal":"TooManyCommands","payload193_refusal":"PayloadTooLong",
            "scene_unchanged_after_refusals":true,"raster_commands":48,
            "raster_pixels_written":raster_pixels,"scope":"root-production-typography-raster",
            "diagnostic_show":false,"protected_scene_interpretation":false
        },
        "surfaces":{
            "mask_surfaces":1,"extra_admitted":7,"extra_uploaded":7,"retained_total":8,
            "ninth_refusal":"OwnerError::Capacity",
            "guest_entries_before_ninth":guest_entries,"guest_entries_after_ninth":guest_entries,
            "same_session":true,"same_service_instance":true,"extra_removed":7,
            "source_face_unchanged":true,"show_unchanged":true,"draft_revision_unchanged":true,
            "keyboard_after_recompose":true,"recompose_pixels_written":recompose_pixels,
            "scope":"root-fixture-session-capacity","mask_plan_reserves_eight":false
        },
        "physical_scanout":false,"ordinary_product":false
    })
}

pub(super) fn validate(transcript: &str) -> Result<Value, ConduitosError> {
    let refusal = || {
        ConduitosError::refusal(
        "compositor-pressure-mismatch",
        "requires exact scene refusal and same-session surface capacity evidence; no diagnostic Show, physical scanout or ordinary-product claim",
    )
    };
    let mut lines = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX));
    let value: Value =
        serde_json::from_str(lines.next().ok_or_else(refusal)?).map_err(|_| refusal())?;
    let positive = |pointer| {
        value
            .pointer(pointer)
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or_else(refusal)
    };
    let pixels = positive("/scene/raster_pixels_written")?;
    let entries = positive("/surfaces/guest_entries_before_ninth")?;
    let recompose = positive("/surfaces/recompose_pixels_written")?;
    if lines.next().is_some() || value != expected(pixels, entries, recompose) {
        return Err(refusal());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(value: &Value) -> String {
        format!("{PREFIX}{value}\n")
    }

    #[test]
    fn exact_capacity_and_proof_boundaries_are_required() {
        let value = expected(1234, 51, 640 * 480);
        let line = record(&value);
        assert_eq!(validate(&line).unwrap(), value);
        for bad in [
            String::new(),
            format!("{line}{line}"),
            format!("{PREFIX}{{\n"),
        ] {
            assert!(validate(&bad).is_err());
        }
        for pointer in [
            "/scene/commands",
            "/scene/text_bytes_per_command",
            "/scene/command49_refusal",
            "/scene/payload193_refusal",
            "/scene/scene_unchanged_after_refusals",
            "/scene/raster_commands",
            "/scene/scope",
            "/scene/diagnostic_show",
            "/scene/protected_scene_interpretation",
            "/surfaces/extra_admitted",
            "/surfaces/extra_uploaded",
            "/surfaces/retained_total",
            "/surfaces/ninth_refusal",
            "/surfaces/same_session",
            "/surfaces/same_service_instance",
            "/surfaces/extra_removed",
            "/surfaces/source_face_unchanged",
            "/surfaces/show_unchanged",
            "/surfaces/draft_revision_unchanged",
            "/surfaces/keyboard_after_recompose",
            "/surfaces/mask_plan_reserves_eight",
            "/physical_scanout",
            "/ordinary_product",
        ] {
            let mut bad = value.clone();
            *bad.pointer_mut(pointer).unwrap() = Value::Null;
            assert!(validate(&record(&bad)).is_err(), "missing {pointer}");
        }
        for pointer in [
            "/scene/raster_pixels_written",
            "/surfaces/guest_entries_before_ninth",
            "/surfaces/recompose_pixels_written",
        ] {
            let mut bad = value.clone();
            *bad.pointer_mut(pointer).unwrap() = Value::from(0);
            assert!(validate(&record(&bad)).is_err(), "zero {pointer}");
        }
        let mut bad = value.clone();
        bad["surfaces"]["guest_entries_after_ninth"] = Value::from(52);
        assert!(validate(&record(&bad)).is_err());
        let mut bad = value;
        bad["physical_scanout"] = Value::Bool(true);
        assert!(validate(&record(&bad)).is_err());
    }
}
