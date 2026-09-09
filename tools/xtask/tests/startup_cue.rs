use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "conduit-startup-cue-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn cue_render_is_a_dry_runnable_new_file_operation_with_honest_evidence() {
    let directory = Directory::new();
    let path = directory.0.join("cue.wav");
    let invoke = |dry_run| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command.arg("--json");
        if dry_run {
            command.arg("--dry-run");
        }
        command
            .args(["audio", "render-startup-cue", "--output"])
            .arg(&path)
            .output()
            .unwrap()
    };
    let dry = invoke(true);
    assert!(dry.status.success(), "{:?}", dry.stderr);
    let report: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(report["effects_performed"], false);
    assert_eq!(report["file_written"], false);
    assert!(!path.exists());
    let actual = invoke(false);
    assert!(actual.status.success(), "{:?}", actual.stderr);
    let report: serde_json::Value = serde_json::from_slice(&actual.stdout).unwrap();
    assert_eq!(report["proof_class"], "deterministic-dsp-render");
    assert_eq!(report["audio_device_opened"], false);
    assert_eq!(report["file_written"], true);
    assert_eq!(report["frames"], 57_600);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        48_000
    );
    assert_eq!(
        u32::from_le_bytes(bytes[40..44].try_into().unwrap()),
        115_200
    );
    assert_eq!(bytes.len(), 115_244);
    assert!(
        !invoke(false).status.success(),
        "existing file must refuse overwrite"
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
