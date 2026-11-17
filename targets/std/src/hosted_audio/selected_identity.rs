//! Current selected sysfs basename with caller-owned Linux realpath scratch.
use std::{ffi::CString, fmt::Write, path::Path};
#[cfg(target_os = "linux")]
use std::{
    ffi::{CStr, OsStr},
    os::unix::ffi::OsStrExt,
};
#[cfg(target_os = "linux")]
const REALPATH_BYTES: usize = libc::PATH_MAX as usize;
#[cfg(not(target_os = "linux"))]
const REALPATH_BYTES: usize = 0;
pub(super) struct SelectedIdentity {
    path: CString,
    fallback: String,
    #[cfg(target_os = "linux")]
    scratch: [libc::c_char; REALPATH_BYTES],
}
impl SelectedIdentity {
    pub(super) fn prepare(card_index: u16) -> Result<Self, &'static str> {
        Self::prepare_at(Path::new("/sys/class/sound"), card_index)
    }
    fn prepare_at(class: &Path, card_index: u16) -> Result<Self, &'static str> {
        if !cfg!(target_os = "linux") {
            return Err("funded speaker identity requires Linux realpath contract");
        }
        let class = class.to_str().ok_or("speaker sysfs path is not UTF-8")?;
        let mut digits = 1usize;
        let mut remaining = card_index;
        while remaining >= 10 {
            remaining /= 10;
            digits += 1;
        }
        let path_len = class
            .len()
            .checked_add("/card/device".len() + digits)
            .ok_or("speaker sysfs path overflow")?;
        if path_len
            .checked_add(1)
            .is_none_or(|bytes| bytes > REALPATH_BYTES)
        {
            return Err("speaker sysfs input exceeds PATH_MAX");
        }
        let mut path = String::with_capacity(path_len + 1);
        write!(&mut path, "{class}/card{card_index}/device").expect("prepared sysfs extent");
        let path = CString::new(path).map_err(|_| "speaker sysfs input contains NUL")?;
        let mut fallback = String::with_capacity("alsa-card-".len() + digits);
        write!(&mut fallback, "alsa-card-{card_index}").expect("prepared fallback extent");
        Ok(Self {
            path,
            fallback,
            #[cfg(target_os = "linux")]
            scratch: [0; REALPATH_BYTES],
        })
    }
    pub(super) fn retained_backing_bytes(&self) -> usize {
        self.path.as_bytes_with_nul().len() + self.fallback.capacity()
    }
    pub(super) fn matches(&mut self, expected: &str) -> bool {
        #[cfg(target_os = "linux")]
        {
            // Linux PATH_MAX is the supported caller-buffer realpath contract.
            // The input is prebuilt, output remains in this admitted fixed array.
            let result = unsafe { libc::realpath(self.path.as_ptr(), self.scratch.as_mut_ptr()) };
            if !result.is_null() {
                let bytes = unsafe { CStr::from_ptr(self.scratch.as_ptr()) }.to_bytes();
                if let Some(name) = Path::new(OsStr::from_bytes(bytes))
                    .file_name()
                    .and_then(|name| name.to_str())
                {
                    return name == expected;
                }
            }
        }
        self.fallback == expected
    }
}
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn matches_existing_canonical_basename_and_fallback_semantics() {
        let root = tempfile::tempdir().unwrap();
        let class = root.path().join("class");
        std::fs::create_dir_all(class.join("card2")).unwrap();
        let device = root.path().join("physical-device");
        std::fs::create_dir(&device).unwrap();
        std::os::unix::fs::symlink(&device, class.join("card2/device")).unwrap();
        let mut identity = SelectedIdentity::prepare_at(&class, 2).unwrap();
        assert!(identity.matches("physical-device"));
        assert!(!identity.matches("other"));
        std::fs::remove_file(class.join("card2/device")).unwrap();
        let unusual = root.path().join(OsStr::from_bytes(b"ancestor-\xff"));
        std::fs::create_dir(&unusual).unwrap();
        let nested = unusual.join("valid-basename");
        std::fs::create_dir(&nested).unwrap();
        std::os::unix::fs::symlink(&nested, class.join("card2/device")).unwrap();
        assert!(identity.matches("valid-basename"));
        std::fs::remove_file(class.join("card2/device")).unwrap();
        assert!(identity.matches("alsa-card-2"));
        assert!(!identity.matches("physical-device"));
    }
}
