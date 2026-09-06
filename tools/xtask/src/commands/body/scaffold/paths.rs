use std::{
    fs,
    io::Write as _,
    path::{Component, Path, PathBuf},
};

pub(super) fn output_path(
    root: &Path,
    name: &str,
    requested: Option<&Path>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let current = std::env::current_dir()?;
    let path = requested.map_or_else(
        || root.join("bodies").join(name).join("main.body.conduit"),
        |path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                current.join(path)
            }
        },
    );
    if !path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.ends_with(".body.conduit"))
    {
        return Err("body new output must end in '.body.conduit'".into());
    }
    Ok(normalize(&path))
}

pub(super) fn write_new(
    path: &Path,
    source: &[u8],
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                format!("refusing to replace existing {kind} {}", path.display())
            } else {
                format!("cannot create {kind} {}: {error}", path.display())
            }
        })?;
    file.write_all(source)?;
    Ok(())
}

pub(super) fn ensure_absent(path: &Path, kind: &str) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        Err(format!("refusing to replace existing {kind} {}", path.display()).into())
    } else {
        Ok(())
    }
}

pub(super) fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from = normalize(from);
    let to = normalize(to);
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let common = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return to;
    }
    let mut relative = PathBuf::new();
    for _ in common..from_components.len() {
        relative.push("..");
    }
    for component in &to_components[common..] {
        relative.push(component.as_os_str());
    }
    relative
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if result.file_name().is_some_and(|name| name != "..") => {
                result.pop();
            }
            _ => result.push(component.as_os_str()),
        }
    }
    result
}

pub(super) fn path_text(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| "Body configuration path is not valid UTF-8".into())
}

pub(super) fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|current| path.strip_prefix(current).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}
