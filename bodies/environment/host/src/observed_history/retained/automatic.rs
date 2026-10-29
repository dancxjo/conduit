//! Automatic read-only projection follows one independently pinned producer Boot.
use super::*;

pub(crate) struct Projection<'a> {
    pub(crate) producer_boot: &'a str,
    pub(crate) output: &'a Path,
}
impl Projection<'_> {
    /// Names are untrusted candidates. Their exact Body/Boot and original Play
    /// become usable only after the complete prefix verifies under admitted Root.
    /// Ambiguous Plays refuse rather than inventing a "latest" authority.
    pub(crate) fn refresh(&self, binding: &Path, directory: &Path) -> Result<()> {
        if !self.producer_boot.starts_with("boot/")
            || self.producer_boot.len() > 128
            || self.producer_boot.bytes().any(|b| b.is_ascii_control())
        {
            return Err("historical producer Boot".into());
        }
        let admitted = crate::local_recorder::validated_binding(binding)?;
        private(directory, true)?;
        let body = Sha256::digest(admitted.credential().body_id.as_str().as_bytes());
        let boot = Sha256::digest(self.producer_boot.as_bytes());
        let mut selected = None;
        for (index, entry) in fs::read_dir(directory)?.enumerate() {
            // Projection work is finite even for a corrupt/unrelated local tree.
            if index == 960 {
                return Err("retained projection directory bound".into());
            }
            let path = entry?.path();
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix("epoch-"))
            else {
                continue;
            };
            let Ok(bytes) = hex(name) else { continue };
            let Ok(scope) = <[u8; 96]>::try_from(bytes) else {
                continue;
            };
            if scope[..32] != body[..] || scope[32..64] != boot[..] {
                continue;
            }
            if selected.replace(path).is_some() {
                return Err("ambiguous retained producer Play candidates".into());
            }
        }
        let epoch = selected.ok_or("no retained epoch for pinned producer Boot")?;
        // The existing publisher now verifies this one complete immutable
        // prefix exactly once. A name alone never authorizes its contents.
        run(binding, &epoch, self.producer_boot, self.output)
    }
    pub(crate) fn after_attempt(&self, binding: &Path, directory: &Path) {
        if let Err(error) = self.refresh(binding, directory) {
            eprintln!("retained projection unchanged: {error}");
        }
    }
}
