//! Finite pinned offline requests and content-addressed canonical proof storage.
use conduit_core::semantic_digest;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
pub fn digest(bytes: &[u8]) -> String {
    semantic_digest("language/causal-prefix-parity@1", bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn sha(path: &Path) -> String {
    assert!(path.is_absolute());
    assert!(std::fs::metadata(path).unwrap().len() <= 64 * 1024 * 1024);
    let result = Command::new("sha256sum").arg(path).output().unwrap();
    assert!(result.status.success());
    String::from_utf8(result.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}
pub fn load(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
pub fn rows(path: &Path) -> Vec<Value> {
    assert!(std::fs::metadata(path).unwrap().len() <= 16 * 1024 * 1024);
    let values: Vec<_> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(values.len() <= 4096);
    values
}
pub fn u(value: &Value) -> u64 {
    value.as_u64().unwrap()
}
pub fn text(value: &Value) -> &str {
    value.as_str().unwrap()
}
pub fn array<const N: usize>(value: &Value) -> [u64; N] {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(u)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}
pub struct Inputs {
    pub manifest: Value,
    pub paired: Vec<Value>,
    pub stops: Vec<Value>,
    pub hashes: Value,
}
impl Inputs {
    pub fn read(root: &Path, pins: &Path, repository: &Path, pin_base: &Path) -> Self {
        assert!(root.is_absolute() && pins.is_absolute());
        let manifest = load(&root.join("manifest.json"));
        let source_pins = load(pins);
        assert_eq!(
            source_pins["spec_sha256"],
            sha(&pins.parent().unwrap().join("SPEC.md"))
        );
        for (name, expected) in source_pins["files"].as_object().unwrap() {
            assert_eq!(
                expected,
                &json!(sha(&repository.join(name))),
                "Source/trainer pin {name}"
            );
        }
        assert!(pin_base.is_absolute());
        for item in manifest["inputs"].as_object().unwrap().values() {
            assert_eq!(
                item["sha256"],
                sha(&pin_base.join(text(&item["path"])).canonicalize().unwrap())
            );
        }
        assert_eq!(manifest["generator_sha256"], sha(&root.join("generate.py")));
        for (name, expected) in manifest["outputs"].as_object().unwrap() {
            assert_eq!(expected, &json!(sha(&root.join(name))));
        }
        let profile = load(
            &pin_base
                .join(text(&manifest["inputs"]["profile"]["path"]))
                .canonicalize()
                .unwrap(),
        );
        assert_eq!(profile, manifest["fixed_profile"]);
        assert_eq!(profile.as_object().unwrap().len(), 64);
        let paired = rows(&root.join("paired-requests.jsonl"));
        let stops = rows(&root.join("stops.jsonl"));
        assert_eq!(paired.len() as u64, u(&manifest["paired_target_count"]));
        assert_eq!(stops.len() as u64, u(&manifest["prefix_schedule_count"]));
        let membership = root
            .parent()
            .unwrap()
            .join("train-causal-role-preparation/membership-and-proposed-roles.jsonl");
        assert_eq!(manifest["cohort_manifest_sha256"], sha(&membership));
        let cohort: BTreeSet<_> = rows(&membership)
            .iter()
            .map(|v| text(&v["sentence_id"]).to_owned())
            .collect();
        let actual: BTreeSet<_> = paired
            .iter()
            .map(|v| text(&v["sentence_id"]).to_owned())
            .collect();
        assert_eq!(cohort, actual);
        assert_eq!(actual.len(), 394);
        let hashes = json!({"paired_sha256":sha(&root.join("paired-requests.jsonl")),"stops_sha256":sha(&root.join("stops.jsonl")),"manifest_sha256":sha(&root.join("manifest.json")),"source_pins_sha256":sha(pins),"cohort_sha256":sha(&membership),"lexical_owner_sha256":sha(&repository.join("semantics/language/src/lexical.rs")),"revision_owner_sha256":sha(&repository.join("semantics/language/src/parser_revision.rs")),"kernel_helper_sha256":sha(&repository.join("semantics/language/tests/common/parser_kernel.rs")),"io_helper_sha256":sha(&repository.join("semantics/language/tests/common/parser_selection_io.rs")),"values_helper_sha256":sha(&repository.join("semantics/language/tests/common/parser_selection_values.rs"))});
        Self {
            manifest,
            paired,
            stops,
            hashes,
        }
    }
}
pub struct Store {
    root: PathBuf,
    events: File,
    pub bytes: u64,
    pub blobs: usize,
}
impl Store {
    pub fn new(root: PathBuf) -> Self {
        assert!(root.is_absolute());
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("canonical")).unwrap();
        let events = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("source-observations.jsonl"))
            .unwrap();
        Self {
            root,
            events,
            bytes: 0,
            blobs: 0,
        }
    }
    pub fn blob(&mut self, bytes: &[u8]) -> String {
        assert!(bytes.len() <= 65536);
        let id = digest(bytes);
        let path = self.root.join("canonical").join(&id);
        if path.exists() {
            assert_eq!(std::fs::read(path).unwrap(), bytes);
            return id;
        }
        self.bytes = self.bytes.checked_add(bytes.len() as u64).unwrap();
        self.blobs += 1;
        assert!(
            self.bytes <= 1024 * 1024 * 1024 && self.blobs <= 50000,
            "explicit local evidence profile exceeded"
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
        id
    }
    pub fn observation(&mut self, value: Value) {
        let line = serde_json::to_vec(&value).unwrap();
        assert!(line.len() <= 16384);
        self.events.write_all(&line).unwrap();
        self.events.write_all(b"\n").unwrap();
    }
    pub fn write(&self, name: &str, value: &Value) {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.root.join(name))
            .unwrap();
        file.write_all(&serde_json::to_vec_pretty(value).unwrap())
            .unwrap();
    }
    pub fn stream(&self, name: &str) -> File {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.root.join(name))
            .unwrap()
    }
}
