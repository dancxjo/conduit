//! Private retained-evidence projection; never a current Face, membership or commit receipt.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    custody: String,
    body_id: String,
    producer_host: String,
    producer_boot: String,
    scope_hex: String,
    forms_hex: String,
    root_public_key_hex: String,
    verified_through_sequence: u32,
    samples_outside_view: u32,
    records_hex: String,
}
fn hex(text: &str, maximum: usize) -> Result<Vec<u8>, String> {
    if text.len() > maximum * 2
        || !text.len().is_multiple_of(2)
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("history hex bounds".into());
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|b| {
            u8::from_str_radix(core::str::from_utf8(b).map_err(|_| "history hex")?, 16)
                .map_err(|_| "history hex".into())
        })
        .collect()
}
/// Exact bounded bytes own one immutable prepared scene. Unit preference is
/// fixed for this preparation; changing it requires a new HistoryInput.
pub struct HistoryInput {
    accepted: Option<(Vec<u8>, conduit_face::GraphicsScene, String)>,
    preferred: Option<conduit_core::Unit>,
}
impl HistoryInput {
    pub fn new(preferred: Option<conduit_core::Unit>) -> Self {
        Self {
            accepted: None,
            preferred,
        }
    }
    pub fn scene(&self) -> Option<&conduit_face::GraphicsScene> {
        self.accepted.as_ref().map(|(_, scene, _)| scene)
    }
    pub fn label(&self) -> Option<&str> {
        self.accepted.as_ref().map(|(_, _, label)| label.as_str())
    }
    pub fn refresh(&mut self, path: &Path) -> Result<bool, String> {
        self.accept(read_bytes(path)?)
    }
    fn accept(&mut self, bytes: Vec<u8>) -> Result<bool, String> {
        if self
            .accepted
            .as_ref()
            .is_some_and(|(original, _, _)| original == &bytes)
        {
            return Ok(false);
        }
        let document: Document = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let host: String = document.producer_host.chars().take(24).collect();
        let boot: String = document
            .producer_boot
            .strip_prefix("boot/")
            .unwrap_or(&document.producer_boot)
            .chars()
            .take(12)
            .collect();
        let mut label = format!("RETAINED HISTORY / {host} / Boot {boot}");
        while label.len() > 112 {
            label.pop();
        }
        let prepared = validate(document, self.preferred)?;
        self.accepted = Some((bytes, prepared, label));
        Ok(true)
    }
}
fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o600
        || metadata.len() > 8192
    {
        return Err("private bounded retained history required".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(8193)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8192 {
        return Err("history extent".into());
    }
    Ok(bytes)
}
fn validate(
    document: Document,
    preferred: Option<conduit_core::Unit>,
) -> Result<conduit_face::GraphicsScene, String> {
    if document.schema != "conduit.environment/retained-observation-series@1"
        || document.custody != "retained-signed-evidence"
        || [
            &document.body_id,
            &document.producer_host,
            &document.producer_boot,
        ]
        .iter()
        .any(|s| s.is_empty() || s.len() > 256)
    {
        return Err("history profile".into());
    }
    let scope = hex(&document.scope_hex, 96)?;
    let root = hex(&document.root_public_key_hex, 32)?;
    let forms = hex(&document.forms_hex, 32)?;
    if scope.len() != 96
        || root.len() != 32
        || root.iter().all(|b| *b == 0)
        || scope[64..].iter().all(|b| *b == 0)
        || scope[..32] != Sha256::digest(document.body_id.as_bytes())[..]
        || scope[32..64] != Sha256::digest(document.producer_boot.as_bytes())[..]
    {
        return Err("history Body/Boot scope".into());
    }
    let contract = conduit_environment_runtime::observation::ObservationContract::prepare()
        .map_err(|_| "history contract")?;
    let descriptor = conduit_environment_runtime::observed_record::descriptor(&contract)
        .map_err(|_| "history forms")?;
    if forms.as_slice() != &Sha256::digest(descriptor)[..] {
        return Err("history forms mismatch".into());
    }
    let records = hex(
        &document.records_hex,
        16 * conduit_environment_runtime::observed_record::BYTES,
    )?;
    let graph = conduit_environment_history::RetainedEvidenceGraph::prepare(
        &records,
        document.verified_through_sequence,
        document.samples_outside_view,
    )
    .map_err(|e| format!("history records: {e:?}"))?;
    graph
        .native_scene_in_unit(preferred.unwrap_or(conduit_core::Unit::Celsius))
        .map_err(|e| format!("history scene: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex_string(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
    fn document() -> Document {
        let body = "body/actual";
        let boot = "boot/actual";
        let mut scope = [1; 96];
        scope[..32].copy_from_slice(&Sha256::digest(body.as_bytes()));
        scope[32..64].copy_from_slice(&Sha256::digest(boot.as_bytes()));
        let descriptor = conduit_environment_runtime::observed_record::descriptor(
            &conduit_environment_runtime::observation::ObservationContract::prepare().unwrap(),
        )
        .unwrap();
        let record = conduit_environment_runtime::observed_record::ObservedRecord {
            sequence: 1,
            observed_at_boot_ms: 1500,
            disposition: conduit_environment_runtime::recording::RecordingDisposition::Available,
            quantities: Some((
                conduit_core::Quantity::new(20, conduit_core::Unit::Celsius),
                conduit_core::Quantity::new(50, conduit_core::Unit::Percent),
            )),
        }
        .encode()
        .unwrap();
        Document {
            schema: "conduit.environment/retained-observation-series@1".into(),
            custody: "retained-signed-evidence".into(),
            body_id: body.into(),
            producer_host: "host/actual".into(),
            producer_boot: boot.into(),
            scope_hex: hex_string(&scope),
            forms_hex: hex_string(&Sha256::digest(descriptor)),
            root_public_key_hex: hex_string(&[9; 32]),
            verified_through_sequence: 1,
            samples_outside_view: 0,
            records_hex: hex_string(&record),
        }
    }
    #[test]
    fn actual_forms_and_body_boot_scope_required_without_claiming_signature_verification() {
        assert!(validate(document(), None).is_ok());
        let mut foreign = document();
        foreign.producer_boot = "boot/foreign".into();
        assert!(validate(foreign, None).is_err());
        let mut forms = document();
        forms.forms_hex = "00".repeat(32);
        assert!(validate(forms, None).is_err());
        let mut commitment = document();
        commitment.custody = "committed-recording".into();
        assert!(validate(commitment, None).is_err());
    }
    #[test]
    fn immutable_scene_reused_for_exact_bytes_and_bad_successor_retains_old_artifact() {
        let bytes = serde_json::to_vec(&document()).unwrap();
        let mut input = HistoryInput::new(Some(conduit_core::Unit::Fahrenheit));
        assert!(input.accept(bytes.clone()).unwrap());
        assert!(input.label().unwrap().contains("host/actual"));
        assert!(input.label().unwrap().contains("actual"));
        let original = input.scene().unwrap().encode();
        let identity = input.scene().unwrap() as *const _;
        assert!(!input.accept(bytes).unwrap());
        assert_eq!(identity, input.scene().unwrap() as *const _);
        assert!(input.accept(b"malformed".to_vec()).is_err());
        assert_eq!(input.scene().unwrap().encode(), original);
        let mut next = document();
        next.samples_outside_view = 22;
        assert!(input.accept(serde_json::to_vec(&next).unwrap()).unwrap());
        assert_ne!(input.scene().unwrap().encode(), original);
        let mut another_source = document();
        another_source.producer_host = "host/second".into();
        assert!(
            input
                .accept(serde_json::to_vec(&another_source).unwrap())
                .unwrap()
        );
        assert!(input.label().unwrap().contains("host/second"));
    }
}

#[cfg(test)]
mod private_preview {
    #[test]
    #[ignore = "requires explicitly selected private verified historical evidence"]
    fn render_actual_retained_history_without_framebuffer_or_services() {
        use std::io::Write;
        let input =
            std::env::var_os("CONDUIT_PRIVATE_HISTORY_PREVIEW").expect("private history input");
        let output = std::env::var_os("CONDUIT_PRIVATE_HISTORY_PREVIEW_PPM")
            .expect("private preview output");
        let mut history = super::HistoryInput::new(Some(conduit_core::Unit::Fahrenheit));
        assert!(history.refresh(std::path::Path::new(&input)).unwrap());
        let (width, height) = (1024u16, 600u16);
        let mut pixels = vec![0; usize::from(width) * usize::from(height)];
        crate::view::render_graphics(history.scene().unwrap(), width, height, &mut pixels).unwrap();
        crate::view::render_notice(
            &format!("{} / READ ONLY", history.label().unwrap()),
            width,
            height,
            &mut pixels,
        )
        .unwrap();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)
            .unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        for pixel in pixels {
            file.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8])
                .unwrap();
        }
        file.sync_all().unwrap();
    }
}
