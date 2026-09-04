//! Bounded protected-file adapter for maker-authored environment documents.

use patchbay_model::AuthoredEnvironment;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const MAX_ENVIRONMENT_BYTES: u64 = 64 * 1024;

pub(super) fn open_environment_resource(path: &PathBuf) -> Result<AuthoredEnvironment, String> {
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err("authored environment paths must end in .json".into());
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() || metadata.len() > MAX_ENVIRONMENT_BYTES {
                return Err("authored environment is not one bounded regular file".into());
            }
            let bytes = fs::read(path).map_err(|error| error.to_string())?;
            let environment: AuthoredEnvironment =
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            environment.validate().map_err(|error| error.to_string())?;
            Ok(environment)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let id = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or("environment path has no UTF-8 file stem")?;
            AuthoredEnvironment::new(id).map_err(|error| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn save_environment_resource(
    path: &Path,
    environment: &AuthoredEnvironment,
) -> Result<(), String> {
    environment.validate().map_err(|error| error.to_string())?;
    let encoded = serde_json::to_vec_pretty(environment).map_err(|error| error.to_string())?;
    if encoded.len() as u64 > MAX_ENVIRONMENT_BYTES {
        return Err("authored environment exceeds its finite byte bound".into());
    }
    let temporary = path.with_extension("environment-save");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    if let Err(error) = file.write_all(&encoded).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        error.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use patchbay_model::{AuthoredPart, MachineProfile};

    #[test]
    fn new_environment_saves_and_reopens_as_bounded_authored_truth() {
        let path =
            std::env::temp_dir().join(format!("patchbay-environment-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut environment = open_environment_resource(&path).unwrap();
        environment
            .add_part(AuthoredPart::reviewed(
                "pico",
                "Pico W",
                MachineProfile::PicoW,
            ))
            .unwrap();
        save_environment_resource(&path, &environment).unwrap();
        assert_eq!(open_environment_resource(&path).unwrap(), environment);
        fs::remove_file(path).unwrap();
    }
}
