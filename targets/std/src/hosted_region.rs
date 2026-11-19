//! Resolve the selected viewer's temperature convention once during preparation.
use conduit_core::Unit;
use conduit_host_make::CheckedHostConfiguration;

pub(crate) fn viewer_temperature(
    checked: &CheckedHostConfiguration,
) -> Result<Option<Unit>, String> {
    let mut selected = checked
        .configuration()
        .bases
        .iter()
        .filter(|b| b.kind == "locale/regional-preferences");
    let Some(base) = selected.next() else {
        return Ok(None);
    };
    if selected.next().is_some()
        || !checked
            .resolved_bases()
            .iter()
            .any(|(kind, implementation)| {
                kind == &base.kind && implementation == "hosted/linux-regional-preferences@1"
            })
    {
        return Err("viewer requires one checked regional Base".into());
    }
    let init = base
        .initialization
        .as_ref()
        .ok_or("regional Base has no checked initialization")?;
    if init.values.len() != 1 {
        return Err("regional Base requires one temperature selection".into());
    }
    match init
        .values
        .get("temperature")
        .and_then(serde_json::Value::as_str)
    {
        Some("celsius") => Ok(Some(Unit::Celsius)),
        Some("fahrenheit") => Ok(Some(Unit::Fahrenheit)),
        Some("system") => system_temperature(),
        _ => Err("unsupported temperature preference".into()),
    }
}

#[cfg(target_env = "gnu")]
fn system_temperature() -> Result<Option<Unit>, String> {
    // glibc langinfo.h defines _NL_MEASUREMENT_MEASUREMENT as the first
    // item in LC_MEASUREMENT. Its value is one byte, independent of encoding.
    // SAFETY: the locale is private and remains alive until the byte is copied.
    let measurement = unsafe {
        let locale = libc::newlocale(
            libc::LC_MEASUREMENT_MASK,
            c"".as_ptr(),
            std::ptr::null_mut(),
        );
        if locale.is_null() {
            return Ok(None);
        }
        let value = libc::nl_langinfo_l(libc::LC_MEASUREMENT << 16, locale)
            .as_ref()
            .copied();
        libc::freelocale(locale);
        value
    };
    Ok(match measurement {
        Some(1) => Some(Unit::Celsius),
        Some(2) => Some(Unit::Fahrenheit),
        _ => None,
    })
}

#[cfg(not(target_env = "gnu"))]
fn system_temperature() -> Result<Option<Unit>, String> {
    Err("system measurement conventions require GNU/Linux".into())
}
