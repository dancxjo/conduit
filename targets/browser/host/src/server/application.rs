//! Generic finite delivery of one already-staged browser application artifact.

use std::path::{Path, PathBuf};

pub(super) const MAX_APPLICATION_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MOUNT_BYTES: usize = 256;

#[derive(Debug)]
pub(super) struct ApplicationDirectory {
    root: PathBuf,
    mount: String,
}

#[derive(Debug)]
pub(super) struct ApplicationResponse {
    pub(super) content_type: &'static str,
    pub(super) body: Vec<u8>,
}

impl ApplicationDirectory {
    pub(super) fn admit(root: &Path, mount: &str) -> Result<Self, String> {
        if !mount.starts_with('/')
            || !mount.ends_with('/')
            || mount.len() > MAX_MOUNT_BYTES
            || mount.contains(['\0', '\r', '\n', '\\', '?', '#', '%'])
            || mount
                .split('/')
                .any(|segment| segment == "." || segment == "..")
        {
            return Err("browser application mount is invalid".into());
        }
        let root = root.canonicalize().map_err(|error| {
            format!(
                "browser application directory {} is unavailable ({error})",
                root.display()
            )
        })?;
        if !root.is_dir() || !root.join("index.html").is_file() {
            return Err("browser application directory has no index.html".into());
        }
        Ok(Self {
            root,
            mount: mount.into(),
        })
    }

    pub(super) fn mount(&self) -> &str {
        &self.mount
    }

    pub(super) fn response(
        &self,
        request_line: Option<&str>,
    ) -> Result<Option<ApplicationResponse>, String> {
        let path = match request_line
            .and_then(|line| line.strip_prefix("GET "))
            .and_then(|line| line.strip_suffix(" HTTP/1.1"))
        {
            Some(path) => path,
            None => return Ok(None),
        };
        let relative = match path.strip_prefix(&self.mount) {
            Some(relative) => relative,
            None => return Ok(None),
        };
        if relative.contains(['\0', '\r', '\n', '\\', '?', '#', '%'])
            || relative
                .split('/')
                .any(|segment| segment == "." || segment == "..")
        {
            return Ok(None);
        }
        let relative = if relative.is_empty() || relative.ends_with('/') {
            format!("{relative}index.html")
        } else {
            relative.into()
        };
        let candidate = self.root.join(&relative);
        let candidate = match candidate.canonicalize() {
            Ok(candidate) => candidate,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("browser application resource failed: {error}")),
        };
        if !candidate.starts_with(&self.root) || !candidate.is_file() {
            return Ok(None);
        }
        let length = candidate
            .metadata()
            .map_err(|error| format!("browser application resource metadata failed: {error}"))?
            .len();
        if length == 0 || length > MAX_APPLICATION_FILE_BYTES {
            return Err(format!(
                "browser application resource exceeds the admitted {MAX_APPLICATION_FILE_BYTES}-byte delivery bound"
            ));
        }
        let body = std::fs::read(&candidate)
            .map_err(|error| format!("browser application resource read failed: {error}"))?;
        Ok(Some(ApplicationResponse {
            content_type: content_type(&candidate),
            body,
        }))
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("mjs" | "js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("md") => "text/markdown; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}
