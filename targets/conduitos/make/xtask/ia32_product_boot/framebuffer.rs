//! Correlate actual bootloader framebuffer facts with the ordinary snapshot.
use super::{complete_line, refusal, ConduitosError};
use serde_json::Value;
const PREFIX: &str = "CONDUIT_IA32_FRAMEBUFFER ";

pub(super) fn capture(
    text: &str,
    product: &Value,
    snapshot: &Value,
) -> Result<Value, ConduitosError> {
    let evidence: Value = serde_json::from_str(complete_line(text, PREFIX).ok_or_else(|| {
        refusal(
            "ia32-framebuffer-observation-missing",
            "complete observation required",
        )
    })?)
    .map_err(|e| invalid(e.to_string()))?;
    validate(&evidence, product, snapshot)?;
    Ok(evidence)
}

fn validate(evidence: &Value, product: &Value, snapshot: &Value) -> Result<(), ConduitosError> {
    let sealed = &snapshot["sealed_boot_provenance"][0];
    let frames = sealed["framebuffers"]
        .as_array()
        .ok_or_else(|| invalid("framebuffers absent"))?;
    let bases = snapshot["bases"]
        .as_array()
        .ok_or_else(|| invalid("Bases absent"))?;
    if evidence["schema"] != "conduit.conduitos/ia32-framebuffer-observation@1"
        || evidence["target"] != "conduitos/ia32/pc"
        || evidence["firmware"] != sealed["firmware_environment"]
        || evidence["graphical_presenter_selected"] != false
        || evidence["graphical_show_proved"] != false
        || evidence.get("framebuffer").is_none()
        || ["profile_id", "build_id", "image_id", "host_id", "boot_id"]
            .iter()
            .any(|key| {
                product[key].as_str().is_none_or(str::is_empty) || evidence[key] != product[key]
            })
    {
        return Err(invalid("wrong product, firmware or proof class"));
    }
    let framebuffer = &evidence["framebuffer"];
    let count = usize::from(!framebuffer.is_null());
    if frames.len() != count || bases.len() != 7 + count {
        return Err(invalid("observed framebuffers and reported Bases disagree"));
    }
    if count == 0 {
        return Ok(());
    }
    let byte = |key: &str| {
        framebuffer[key]
            .as_u64()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| invalid(key))
    };
    let word = |key: &str| {
        framebuffer[key]
            .as_u64()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| invalid(key))
    };
    let format = conduitos::display::DisplayFormat {
        width: word("width")?,
        height: word("height")?,
        pitch: word("pitch_bytes")?,
        bits_per_pixel: byte("bits_per_pixel")?,
        red_shift: byte("red_shift")?,
        green_shift: byte("green_shift")?,
        blue_shift: byte("blue_shift")?,
    }
    .validate()
    .map_err(|e| invalid(e.as_str()))?;
    let bytes = format.byte_len().map_err(|e| invalid(e.as_str()))? as u64;
    let physical = framebuffer["physical_start"]
        .as_u64()
        .ok_or_else(|| invalid("physical address absent"))?;
    if bytes > 8 * 1024 * 1024
        || framebuffer["byte_length"] != bytes
        || physical == 0
        || !physical.is_multiple_of(4)
        || physical
            .checked_add(bytes)
            .is_none_or(|end| end > 1_u64 << 32)
        || [
            "base_id",
            "width",
            "height",
            "pitch_bytes",
            "bits_per_pixel",
        ]
        .iter()
        .any(|key| frames[0][key] != framebuffer[key])
        || framebuffer["base_id"].as_str().is_none_or(str::is_empty)
        || !bases.iter().any(|base| {
            base["base_id"] == framebuffer["base_id"]
                && base["kind_id"] == "conduitos.base/framebuffer@1"
                && base["capacity_units"] == bytes
        })
    {
        return Err(invalid(
            "invalid RGB extent or contradictory Observatory geometry",
        ));
    }
    Ok(())
}
fn invalid(detail: impl Into<String>) -> ConduitosError {
    refusal("ia32-framebuffer-observation-invalid", detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> (Value, Value, Value) {
        let product = json!({"profile_id":"profile","build_id":"build","image_id":"image","host_id":"host","boot_id":"boot"});
        let framebuffer = json!({"base_id":"fb","width":800,"height":600,"pitch_bytes":3200,"bits_per_pixel":32,
            "red_shift":16,"green_shift":8,"blue_shift":0,"physical_start":0xc0000000_u64,"byte_length":1920000});
        let evidence = json!({"schema":"conduit.conduitos/ia32-framebuffer-observation@1","target":"conduitos/ia32/pc",
            "profile_id":"profile","build_id":"build","image_id":"image","host_id":"host","boot_id":"boot",
            "firmware":"uefi32","framebuffer":framebuffer,"graphical_presenter_selected":false,"graphical_show_proved":false});
        let mut bases = vec![Value::Null; 7];
        bases.push(json!({"base_id":"fb","kind_id":"conduitos.base/framebuffer@1","capacity_units":1920000}));
        let snapshot = json!({"bases":bases,"sealed_boot_provenance":[{"firmware_environment":"uefi32",
            "framebuffers":[{"base_id":"fb","width":800,"height":600,"pitch_bytes":3200,"bits_per_pixel":32}]}]});
        (evidence, product, snapshot)
    }
    #[test]
    fn actual_geometry_requires_the_same_boot_and_observatory_base() {
        let (mut evidence, product, mut snapshot) = fixture();
        assert!(validate(&evidence, &product, &snapshot).is_ok());
        evidence["boot_id"] = "foreign".into();
        assert!(validate(&evidence, &product, &snapshot).is_err());
        evidence["boot_id"] = "boot".into();
        snapshot["sealed_boot_provenance"][0]["framebuffers"][0]["width"] = 640.into();
        assert!(validate(&evidence, &product, &snapshot).is_err());
    }
    #[test]
    fn bad_stride_address_or_graphical_claim_refuses() {
        for (field, value) in [
            ("pitch_bytes", json!(1)),
            ("physical_start", json!(1_u64 << 32)),
            ("byte_length", json!(12)),
        ] {
            let (mut evidence, product, snapshot) = fixture();
            evidence["framebuffer"][field] = value;
            assert!(validate(&evidence, &product, &snapshot).is_err());
        }
        let (mut evidence, product, snapshot) = fixture();
        evidence["graphical_show_proved"] = true.into();
        assert!(validate(&evidence, &product, &snapshot).is_err());
    }
    #[test]
    fn absence_and_ega_text_do_not_invent_an_rgb_base() {
        let (mut evidence, product, mut snapshot) = fixture();
        evidence["framebuffer"] = Value::Null;
        snapshot["sealed_boot_provenance"][0]["framebuffers"] = json!([]);
        snapshot["bases"].as_array_mut().unwrap().pop();
        assert!(validate(&evidence, &product, &snapshot).is_ok());
        snapshot["bases"].as_array_mut().unwrap().push(Value::Null);
        assert!(validate(&evidence, &product, &snapshot).is_err());
    }
}
