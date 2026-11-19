//! Local presentation preferences acquired once on explicit Base selection.
//! No process-global locale change or clock adjustment is made.
use conduit_face::TemperatureDisplayUnit;

pub struct RegionalPreferences {
    pub temperature: Option<TemperatureDisplayUnit>,
    timezone: Option<String>,
}

impl RegionalPreferences {
    pub fn timezone(&self) -> Option<&str> {
        self.timezone.as_deref()
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    pub fn observe(temperature_override: Option<TemperatureDisplayUnit>) -> Self {
        Self {
            temperature: temperature_override.or_else(|| locale_temperature(c"")),
            timezone: std::fs::canonicalize("/etc/localtime")
                .ok()
                .as_deref()
                .and_then(zone_name),
        }
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn zone_name(path: &std::path::Path) -> Option<String> {
    let name = path.strip_prefix("/usr/share/zoneinfo").ok()?.to_str()?;
    if name.len() > 128
        || name.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_+-".contains(&b))
        })
    {
        return None;
    }
    Some(name.to_owned())
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn locale_temperature(name: &std::ffi::CStr) -> Option<TemperatureDisplayUnit> {
    // glibc langinfo.h defines _NL_MEASUREMENT_MEASUREMENT as
    // _NL_ITEM(__LC_MEASUREMENT, 0). The result is one byte, not a string.
    let item = libc::LC_MEASUREMENT << 16;
    // SAFETY: The caller supplies a valid C string (empty selects the effective locale).
    // The handle is private, queried synchronously and freed exactly once.
    let measurement = unsafe {
        let locale = libc::newlocale(
            libc::LC_MEASUREMENT_MASK,
            name.as_ptr(),
            std::ptr::null_mut(),
        );
        if locale.is_null() {
            return None;
        }
        let value = libc::nl_langinfo_l(item, locale).as_ref().copied();
        libc::freelocale(locale);
        value
    };
    match measurement {
        Some(1) => Some(TemperatureDisplayUnit::Celsius),
        Some(2) => Some(TemperatureDisplayUnit::Fahrenheit),
        _ => None,
    }
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn explicit_unit_and_unavailable_locale_are_distinct() {
        assert_eq!(
            RegionalPreferences::observe(Some(TemperatureDisplayUnit::Fahrenheit)).temperature,
            Some(TemperatureDisplayUnit::Fahrenheit)
        );
        assert!(locale_temperature(c"conduit-missing-locale").is_none());
        assert_eq!(
            locale_temperature(c"C"),
            Some(TemperatureDisplayUnit::Celsius)
        );
    }

    #[test]
    fn timezone_requires_an_identifiable_zoneinfo_path() {
        assert_eq!(
            zone_name(Path::new("/usr/share/zoneinfo/Europe/London")).as_deref(),
            Some("Europe/London")
        );
        for path in [
            "/etc/localtime",
            "/usr/share/zoneinfo-other/GMT",
            "/usr/share/zoneinfo/Bad\nName",
        ] {
            assert!(zone_name(Path::new(path)).is_none());
        }
    }
}
