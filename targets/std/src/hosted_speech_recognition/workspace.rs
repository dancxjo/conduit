//! One private, non-retained call workspace for a persistent provider.
use super::WhisperFailure;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) fn private_directory(path: &Path) -> Result<(), WhisperFailure> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|_| WhisperFailure::InvalidProvider)
}

pub(super) struct CallFiles {
    root: PathBuf,
}
impl CallFiles {
    pub(super) fn create(parent: &Path) -> Result<Self, WhisperFailure> {
        let root = parent.join("turn");
        // A preceding cleanup failure refuses another call; never read old
        // results or grow an unbounded sequence of abandoned directories.
        private_directory(&root)?;
        Ok(Self { root })
    }
    pub(super) fn input(&self) -> PathBuf {
        self.root.join("input.wav")
    }
    pub(super) fn output_base(&self) -> PathBuf {
        self.root.join("result")
    }
}
impl Drop for CallFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub(super) fn read_transcript(path: &Path, text_limit: u16) -> Result<String, WhisperFailure> {
    // The text limit applies after trimming provider formatting. Admit a
    // separate finite formatting allowance, rather than reading an unbounded file.
    let maximum = u64::from(text_limit) + 4096;
    let file = fs::File::open(path).map_err(|_| WhisperFailure::ReadFailed)?;
    let metadata = file.metadata().map_err(|_| WhisperFailure::ReadFailed)?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(WhisperFailure::OutputOverflow);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WhisperFailure::ReadFailed)?;
    if bytes.len() as u64 > maximum {
        return Err(WhisperFailure::OutputOverflow);
    }
    String::from_utf8(bytes).map_err(|_| WhisperFailure::ReadFailed)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn leftover_workspace_refuses_reuse_and_oversized_transcript_is_bounded() {
        let (root, _, _) = crate::hosted_speech_recognition::tests::fixture("#!/bin/sh\nexit 99\n");
        let turn = root.join("turn");
        private_directory(&turn).unwrap();
        let text = turn.join("result.txt");
        fs::write(&text, b"prior turn").unwrap();
        assert!(matches!(
            CallFiles::create(&root),
            Err(WhisperFailure::InvalidProvider)
        ));
        assert_eq!(fs::read(&text).unwrap(), b"prior turn");
        fs::File::create(&text)
            .unwrap()
            .set_len(1024 * 1024)
            .unwrap();
        assert_eq!(
            read_transcript(&text, 256),
            Err(WhisperFailure::OutputOverflow)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
