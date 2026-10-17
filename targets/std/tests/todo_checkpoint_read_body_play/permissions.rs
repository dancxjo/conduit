//! Actual denied file access through the selected checkpoint read Host Call.
use super::*;
use std::os::unix::fs::PermissionsExt;

struct RestorePermissions(PathBuf, std::fs::Permissions);
impl Drop for RestorePermissions {
    fn drop(&mut self) {
        std::fs::set_permissions(&self.0, self.1.clone()).expect("restore fixture permissions");
    }
}

#[test]
fn permission_denial_is_inaccessible_not_missing_and_never_emits_state() {
    on_body_stack(|| {
        let root = root();
        seed(&root);
        for extension in ["current", "checkpoint"] {
            let file = std::fs::read_dir(&root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| path.extension().is_some_and(|ext| ext == extension))
                .unwrap();
            let guard = RestorePermissions(
                file.clone(),
                std::fs::metadata(&file).unwrap().permissions(),
            );
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
            // This test is a real OS denial, never a fabricated provider result.
            assert_eq!(
                std::fs::File::open(&file).unwrap_err().kind(),
                std::io::ErrorKind::PermissionDenied
            );
            let (failed, fore) = restore(&root, 2);
            assert_ne!(failed.terminal, TerminalDisposition::Completed);
            assert_eq!(
                failed.kernel_failure,
                Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::HostCallFailed,
                    detail: 10,
                })
            );
            assert!(fore.0.is_empty());
            drop(guard);
            let (recovered, fore) = restore(&root, 2);
            assert_eq!(recovered.terminal, TerminalDisposition::Completed);
            assert_eq!(fore.0.len(), 1);
        }
        std::fs::remove_dir_all(root).unwrap();
    });
}

struct RestoreSelector(PathBuf, PathBuf);
impl Drop for RestoreSelector {
    fn drop(&mut self) {
        if self.0.is_dir() {
            std::fs::remove_dir(&self.0).expect("remove temporary selector directory");
        }
        std::fs::rename(&self.1, &self.0).expect("restore selected checkpoint file");
    }
}

#[test]
fn temporary_storage_error_is_distinct_from_missing_and_denied_access() {
    on_body_stack(|| {
        let root = root();
        seed(&root);
        let selector = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().is_some_and(|ext| ext == "current"))
            .unwrap();
        let backup = selector.with_extension("saved-selector");
        std::fs::rename(&selector, &backup).unwrap();
        let guard = RestoreSelector(selector.clone(), backup);
        std::fs::create_dir(&selector).unwrap();
        assert_eq!(
            std::fs::read(&selector).unwrap_err().kind(),
            std::io::ErrorKind::IsADirectory
        );
        let (failed, fore) = restore(&root, 2);
        assert_ne!(failed.terminal, TerminalDisposition::Completed);
        assert_eq!(
            failed.kernel_failure,
            Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallFailed,
                detail: 8,
            })
        );
        assert!(fore.0.is_empty());
        drop(guard);
        let (recovered, fore) = restore(&root, 2);
        assert_eq!(recovered.terminal, TerminalDisposition::Completed);
        assert_eq!(fore.0.len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    });
}
