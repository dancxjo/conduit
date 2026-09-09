use super::{refusal, ConduitosError};
use serde_json::Value;

pub(super) fn validate_symbols(symbols: &str) -> Result<(), ConduitosError> {
    if !symbols
        .lines()
        .any(|line| line.ends_with(" conduitos_start"))
    {
        return Err(refusal(
            "headless-entry-absent",
            "final ELF has no ordinary boot entry",
        ));
    }
    for forbidden in [
        "conduitos::native_compositor",
        "conduitos::native_typography",
        "conduitos::graphical_startup",
        "conduitos::product_front_door",
        "conduitos::display::",
        "ttf_parser::",
        "fontdue::",
        "rustybuzz::",
        "FONT_BYTES",
        "FONT_DATA",
    ] {
        if symbols.contains(forbidden) {
            return Err(refusal("headless-graphical-symbol-leaked", forbidden));
        }
    }
    Ok(())
}

pub(super) fn validate(
    serial: &str,
    profile: &str,
    build: &str,
    binding: &str,
) -> Result<Value, ConduitosError> {
    let invalid = || {
        refusal(
            "headless-startup-contract-invalid",
            "exact unsupported provenance and absence of initialized offers/workload are required",
        )
    };
    let lines = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_HEADLESS_STARTUP "))
        .collect::<Vec<_>>();
    if lines.len() != 1
        || [
            "CONDUIT_BOOT_SIGN",
            "CONDUIT_KERNEL_SIGN",
            "CONDUIT_PRODUCT_JOURNEY",
            "CONDUIT_PRESENTATION_SIGN",
            "CONDUIT_BOOT_STAGE xhci",
        ]
        .iter()
        .any(|prefix| serial.contains(prefix))
    {
        return Err(invalid());
    }
    let sign: Value = serde_json::from_str(lines[0]).map_err(|_| invalid())?;
    for (field, expected) in [
        ("schema", "conduit.conduitos/headless-startup@1"),
        ("status", "unsupported"),
        ("reason", "headless-workload-entry-unavailable"),
        ("profile_id", profile),
        ("build_id", build),
        ("image_binding", binding),
    ] {
        if sign[field] != expected {
            return Err(invalid());
        }
    }
    for field in ["host_id", "boot_id"] {
        if !sign[field].as_str().is_some_and(|id| {
            id.len() == 64
                && id.bytes().all(|b| b.is_ascii_hexdigit())
                && id.bytes().any(|b| b != b'0')
        }) {
            return Err(invalid());
        }
    }
    for field in ["offer_generation", "body_id", "plan_id", "active_play_id"] {
        if sign.get(field) != Some(&Value::Null) {
            return Err(invalid());
        }
    }
    for field in [
        "initialized_capabilities",
        "presenters",
        "facilities",
        "presentation_surface_slots",
        "presentation_surface_bytes",
        "allocated_bytes",
    ] {
        if sign[field] != 0 {
            return Err(invalid());
        }
    }
    let arena = sign["runtime_arena_bytes"].as_u64().ok_or_else(invalid)?;
    let ceiling = sign["runtime_arena_ceiling"].as_u64().ok_or_else(invalid)?;
    let implementations = sign["compiled_implementations"]
        .as_u64()
        .ok_or_else(invalid)?;
    if arena == 0
        || arena > ceiling
        || implementations == 0
        || implementations & u64::from(conduitos::fabrication::IMPL_NATIVE_PRESENTER) != 0
        || sign["bounded"] != true
    {
        return Err(invalid());
    }
    Ok(sign)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn sign() -> Value {
        json!({"schema":"conduit.conduitos/headless-startup@1","status":"unsupported","reason":"headless-workload-entry-unavailable",
            "profile_id":"profile","build_id":"build","image_binding":"image","host_id":"1".repeat(64),"boot_id":"2".repeat(64),
            "compiled_implementations":31,"initialized_capabilities":0,"offer_generation":null,"body_id":null,"plan_id":null,"active_play_id":null,
            "presenters":0,"facilities":0,"presentation_surface_slots":0,"presentation_surface_bytes":0,"runtime_arena_bytes":262144,"runtime_arena_ceiling":8388608,"allocated_bytes":0,"bounded":true})
    }
    fn transcript(value: &Value) -> String {
        format!("CONDUIT_HEADLESS_STARTUP {value}\n")
    }
    #[test]
    fn headless_proof_rejects_false_success_graphics_and_invented_workload() {
        let valid = sign();
        assert!(validate(&transcript(&valid), "profile", "build", "image").is_ok());
        for (field, value) in [
            ("status", json!("ready")),
            ("reason", json!("panic")),
            ("build_id", json!("other")),
            ("host_id", json!("0".repeat(64))),
            ("body_id", json!("body")),
            ("plan_id", json!("plan")),
            ("active_play_id", json!("play")),
            ("offer_generation", json!(1)),
            ("initialized_capabilities", json!(1)),
            ("presenters", json!(1)),
            ("allocated_bytes", json!(1)),
            ("runtime_arena_ceiling", json!(1)),
            ("compiled_implementations", json!(256)),
        ] {
            let mut changed = valid.clone();
            changed[field] = value;
            assert!(
                validate(&transcript(&changed), "profile", "build", "image").is_err(),
                "{field}"
            );
        }
        for suffix in [
            transcript(&valid),
            "CONDUIT_PRODUCT_JOURNEY {}\n".into(),
            "CONDUIT_KERNEL_SIGN {}\n".into(),
        ] {
            assert!(
                validate(&(transcript(&valid) + &suffix), "profile", "build", "image").is_err()
            );
        }
    }
    #[test]
    fn headless_proof_requires_real_entry_and_excludes_linked_graphical_symbols() {
        assert!(validate_symbols("0000 T conduitos_start\n").is_ok());
        assert!(validate_symbols("").is_err());
        for name in [
            "conduitos::native_compositor::run",
            "conduitos::native_typography::FONT_DATA",
            "ttf_parser::Face::parse",
        ] {
            assert!(validate_symbols(&format!("0000 T conduitos_start\n0001 T {name}\n")).is_err());
        }
    }
}
