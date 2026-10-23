//! A live file read can end inside a UTF-8 scalar or a guest record.
//! Observe only newline-completed records; malformed completed data still fails.
use std::{fs, io, path::Path, str::Utf8Error};

fn complete_end(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1)
}

pub(super) fn complete_records(bytes: &[u8]) -> Result<&str, Utf8Error> {
    std::str::from_utf8(&bytes[..complete_end(bytes)])
}

pub(super) fn read_complete(path: &Path) -> io::Result<String> {
    let mut bytes = fs::read(path)?;
    bytes.truncate(complete_end(&bytes));
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Snapshot(std::path::PathBuf);
    impl Snapshot {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "conduit-live-serial-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            Self(path)
        }
    }
    impl Drop for Snapshot {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn live_file_reads_wait_for_complete_multibyte_records() {
        let file = Snapshot::new();
        let record = "CONDUIT_NATIVE_FACE {\"name\":\"Lương Quốc Lâm é中🦀\"}\n";
        let mut bytes = b"READY\n".to_vec();
        let mut incomplete_utf8_snapshots = 0;
        for byte in record.as_bytes() {
            fs::write(&file.0, &bytes).unwrap();
            incomplete_utf8_snapshots += usize::from(fs::read_to_string(&file.0).is_err());
            assert_eq!(read_complete(&file.0).unwrap(), "READY\n");
            assert_eq!(complete_records(&bytes).unwrap(), "READY\n");
            bytes.push(*byte);
        }
        assert!(incomplete_utf8_snapshots > 0);
        fs::write(&file.0, &bytes).unwrap();
        assert_eq!(read_complete(&file.0).unwrap(), format!("READY\n{record}"));
    }

    #[test]
    fn completed_corruption_and_file_errors_remain_refusals() {
        let file = Snapshot::new();
        fs::write(&file.0, b"READY\n\xff\nunfinished\xe2").unwrap();
        assert_eq!(
            read_complete(&file.0).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(&file.0).unwrap();
        assert_eq!(
            read_complete(&file.0).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }
}
