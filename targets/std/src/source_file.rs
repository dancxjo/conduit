//! Preparation-only bounded acquisition from a Root-selected file descriptor.
use conduit_plot::MAXIMUM_PLOT_SOURCE_BYTES;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

#[derive(Debug)]
pub enum SourceFileRefusal {
    Bounds,
    Scope,
    Metadata(std::io::Error),
    NotRegular,
    Bytes,
    ExtentChanged,
    Read(std::io::Error),
    Utf8,
}
/// Bounded, immutable UTF-8 capture from a descriptor selected by Root. Capture
/// does not select an entry, catalog, generation, or authority. The owning policy
/// can check content identity before admitting any parser or compiler work.
pub struct CapturedSourceFile {
    source: String,
}
impl CapturedSourceFile {
    pub fn from_file(mut file: File, maximum_bytes: usize) -> Result<Self, SourceFileRefusal> {
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_PLOT_SOURCE_BYTES {
            return Err(SourceFileRefusal::Bounds);
        }
        let metadata = file.metadata().map_err(SourceFileRefusal::Metadata)?;
        if !metadata.is_file() {
            return Err(SourceFileRefusal::NotRegular);
        }
        let extent = usize::try_from(metadata.len()).map_err(|_| SourceFileRefusal::Bytes)?;
        if extent > maximum_bytes {
            return Err(SourceFileRefusal::Bytes);
        }
        file.seek(SeekFrom::Start(0))
            .map_err(SourceFileRefusal::Read)?;
        let bytes = capture_extent(&mut file, extent)?;
        let source = String::from_utf8(bytes).map_err(|_| SourceFileRefusal::Utf8)?;
        Ok(Self { source })
    }
    pub fn into_source(self) -> String {
        self.source
    }
    pub fn source(&self) -> &str {
        &self.source
    }
}
// One fixed extent plus an overflow sentinel. Contrary evidence never grows
// storage, silently changes the admitted extent, or starts an unbounded retry.
fn capture_extent(mut input: impl Read, extent: usize) -> Result<Vec<u8>, SourceFileRefusal> {
    if extent > MAXIMUM_PLOT_SOURCE_BYTES {
        return Err(SourceFileRefusal::Bounds);
    }
    let mut bytes = vec![0; extent + 1];
    let mut used = 0;
    while used < bytes.len() {
        match input.read(&mut bytes[used..]) {
            Ok(0) => break,
            Ok(count) => used += count,
            Err(error) => return Err(SourceFileRefusal::Read(error)),
        }
    }
    if used != extent {
        return Err(SourceFileRefusal::ExtentChanged);
    }
    bytes.truncate(used);
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Cursor, Write},
        sync::atomic::{AtomicU64, Ordering},
    };

    fn selected(bytes: &[u8]) -> File {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "conduit-captured-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        std::fs::remove_file(path).unwrap();
        file
    }

    #[test]
    fn captured_unlinked_source_is_exact_without_a_parser_or_path_reopen() {
        let text = "# α 🦀\nunfinished {";
        let captured =
            CapturedSourceFile::from_file(selected(text.as_bytes()), text.len()).unwrap();
        assert_eq!(captured.source(), text);
        assert_eq!(captured.into_source(), text);
        assert!(CapturedSourceFile::from_file(selected(&[]), 1)
            .unwrap()
            .source()
            .is_empty());
    }

    #[test]
    fn source_capture_refuses_invalid_bounds_extent_and_encoding() {
        assert!(matches!(
            CapturedSourceFile::from_file(selected(b"x"), 0),
            Err(SourceFileRefusal::Bounds)
        ));
        assert!(matches!(
            CapturedSourceFile::from_file(selected(b"xx"), 1),
            Err(SourceFileRefusal::Bytes)
        ));
        assert!(matches!(
            CapturedSourceFile::from_file(selected(&[0xff]), 1),
            Err(SourceFileRefusal::Utf8)
        ));
        #[cfg(unix)]
        assert!(matches!(
            CapturedSourceFile::from_file(File::open("/dev/null").unwrap(), 1),
            Err(SourceFileRefusal::NotRegular)
        ));
    }
    #[test]
    fn changed_extent_cannot_grow_storage_and_interruption_is_not_an_unbounded_retry() {
        let mut growing = Cursor::new([1u8; 1024]);
        assert!(matches!(
            capture_extent(&mut growing, 2),
            Err(SourceFileRefusal::ExtentChanged)
        ));
        assert_eq!(
            growing.position(),
            3,
            "only the original extent plus sentinel is read"
        );
        assert!(matches!(
            capture_extent(Cursor::new([1u8]), 2),
            Err(SourceFileRefusal::ExtentChanged)
        ));
        struct Interrupted(usize);
        impl Read for Interrupted {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                self.0 += 1;
                Err(std::io::ErrorKind::Interrupted.into())
            }
        }
        let mut interrupted = Interrupted(0);
        assert!(matches!(
            capture_extent(&mut interrupted, 2),
            Err(SourceFileRefusal::Read(_))
        ));
        assert_eq!(interrupted.0, 1);
    }
}
