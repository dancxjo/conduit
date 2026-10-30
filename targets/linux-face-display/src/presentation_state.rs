//! Private, bounded persistence and acquisition for local Mask preferences.
use crate::{
    mask_controls::{self, Gestures},
    touch_input::TouchInput,
};
use conduit_face::presentation_preferences::{
    PresentationPreferences as Preferences, TemperatureUnit,
};
use std::{
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
    time::{Duration, Instant},
};
pub struct PresentationState {
    pub preferences: Preferences,
    pub context: mask_controls::Context,
    path: Option<PathBuf>,
    physical_height: u16,
    touch: Option<TouchInput>,
    gestures: Gestures,
    changed: Option<Instant>,
    scan: Instant,
}
impl PresentationState {
    pub fn new(width: u16, height: u16, preferred: Option<conduit_core::Unit>) -> Self {
        let path = std::env::var_os("CONDUIT_DISPLAY_PREFERENCES_FILE")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| {
                    PathBuf::from(home).join(".local/state/conduit-face-display/preferences.json")
                })
            });
        let mut preferences = Preferences::default();
        if preferred == Some(conduit_core::Unit::Fahrenheit) {
            preferences.temperature = TemperatureUnit::Fahrenheit;
        }
        if let Some(path) = &path {
            match load(path) {
                Ok(saved) => preferences = saved,
                Err(_error) if !path.exists() => {}
                Err(error) => eprintln!("Presentation settings refused: {error}"),
            }
        }
        Self {
            preferences,
            context: mask_controls::Context::default(),
            physical_height: height,
            path,
            touch: TouchInput::discover(width, height),
            gestures: Gestures::default(),
            changed: None,
            scan: Instant::now(),
        }
    }
    pub fn poll(&mut self, width: u16, height: u16, graph_left: u16) -> bool {
        if self.touch.is_none() && self.scan.elapsed() >= Duration::from_secs(2) {
            self.scan = Instant::now();
            self.touch = TouchInput::discover(width, self.physical_height);
            self.gestures = Gestures::default();
        }
        self.gestures.context = self.context;
        let mut changed = false;
        let divider = mask_controls::divider_position(self.preferences, width);
        if let Some(touch) = &mut self.touch {
            let result = touch.poll(|contacts| {
                if let Some(action) = self.gestures.frame_with_divider(
                    contacts,
                    width,
                    height,
                    graph_left,
                    (width >= 652 && self.context.history).then_some(divider),
                ) {
                    changed |= self.preferences.apply(action);
                }
            });
            if let Err(error) = result {
                eprintln!("Touch input unavailable: {error}");
                self.touch = None;
                self.gestures = Gestures::default();
            }
        }
        if changed {
            self.changed = Some(Instant::now());
        }
        if self
            .changed
            .is_some_and(|time| time.elapsed() >= Duration::from_millis(500))
        {
            self.changed = None;
            if let Some(path) = &self.path
                && let Err(error) = save(path, self.preferences)
            {
                eprintln!("Presentation settings could not be saved: {error}");
            }
        }
        changed
    }
}
fn load(path: &std::path::Path) -> Result<Preferences, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o777 != 0o600
        || meta.len() > 2048
    {
        return Err("expected private regular settings file <=2048B".into());
    }
    let mut bytes = Vec::new();
    file.take(2049)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 2048 {
        return Err("settings extent".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn save(path: &std::path::Path, preferences: Preferences) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("settings parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = path.with_file_name(format!(".preferences-{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(&serde_json::to_vec(&preferences).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_preferences_round_trip_and_unsafe_successors_refuse() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "conduit-presentation-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("preferences.json");
        let mut p = Preferences::default();
        p.apply(conduit_face::presentation_preferences::PresentationAction::ToggleTemperatureUnit);
        p.apply(conduit_face::presentation_preferences::PresentationAction::MoveDivider(1000));
        save(&path, p).unwrap();
        assert_eq!(load(&path).unwrap(), p);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = directory.join("symlink");
        symlink(&path, &link).unwrap();
        assert!(load(&link).is_err());
        std::fs::write(&path, vec![b' '; 2049]).unwrap();
        assert!(load(&path).is_err());
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
