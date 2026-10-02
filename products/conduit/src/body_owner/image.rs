//! Prove the foreground owner is executing the installed product image.
use super::super::{bounded_read, digest, read_installation, MAXIMUM_RELEASE_FILE_BYTES};
use std::path::Path;

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let installation = read_installation(&root.join("installation.json"))?;
    // This entrance is Linux-only. /proc/self/exe references the running image,
    // even if someone has since replaced the pathname used to launch it.
    verify_images(
        Path::new("/proc/self/exe"),
        Path::new(&installation.product_executable),
    )
}

fn verify_images(running: &Path, installed: &Path) -> Result<(), String> {
    let running = digest(&bounded_read(running, MAXIMUM_RELEASE_FILE_BYTES)?);
    let installed = digest(&bounded_read(installed, MAXIMUM_RELEASE_FILE_BYTES)?);
    if running != installed {
        return Err("running executable differs from the installed Host image; launch the installed product executable".into());
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::durable_host::fresh_identity;

    #[test]
    fn image_identity_accepts_exact_bytes_and_refuses_mismatch() {
        let root = std::env::temp_dir().join(fresh_identity("owner-image-test", "identity"));
        std::fs::create_dir_all(&root).unwrap();
        let running = root.join("running");
        let installed = root.join("installed");
        std::fs::write(&running, b"reviewed product image").unwrap();
        std::fs::copy(&running, &installed).unwrap();
        verify_images(&running, &installed).unwrap();
        std::fs::write(&installed, b"different product image").unwrap();
        assert!(verify_images(&running, &installed)
            .unwrap_err()
            .contains("running executable differs"));
        assert!(!root.join("runtime.json").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
