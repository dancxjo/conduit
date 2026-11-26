//! Optional genuine Owner64 → authenticated loopback → production native C ABI.
//! This proves CPU protocol/ABI composition, not Chromium, WASM or physical Show.
use super::*;
use conduit_face::{OwnerFaceSnapshotRequest, OWNER_FACE_REQUEST_SCHEMA};
use std::{fs, path::PathBuf};
mod abi;
mod assertions;
mod carrier;

pub(crate) struct Browser64Proof {
    runtime: abi::Runtime,
    carrier: carrier::Carrier,
    snapshot: BrowserAdmittedSnapshot,
    window: String,
    output: PathBuf,
}
impl Browser64Proof {
    pub(crate) fn prepare_if_requested(owner: &mut Owner, root: &Path) -> Option<Self> {
        let output = PathBuf::from(std::env::var_os(
            "CONDUIT_ENVIRONMENT_HISTORY64_BROWSER_PROOF_DIR",
        )?);
        let allowed = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/environment-recording-source")
            .canonicalize()
            .unwrap();
        assert!(output.is_absolute());
        assert!(output
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(&allowed));
        fs::create_dir(&output).expect("proof directory must be fresh");
        let (runtime, advertisement, key) = abi::Runtime::initialize();
        let mut carrier = carrier::Carrier::open();
        let binding = carrier.socket.binding().clone();
        let authorization = owner
            .browser_authorize_window(
                runtime.host.as_str(),
                Some(key.clone().try_into().unwrap()),
                60_000,
            )
            .unwrap();
        let (advertise, size) = carrier.to_owner(&In::Advertise {
            protocol: PROTOCOL,
            advertisement,
            friendly_label: "Native ABI full64 proof".into(),
            verifying_key: key,
            freshness_sequence: 1,
        });
        let challenge_frame = owner
            .browser_begin(&authorization.window_id, &binding, advertise, size)
            .unwrap();
        let received = carrier.to_browser(&challenge_frame);
        let Out::Challenge { challenge, .. } = serde_json::from_slice(&received).unwrap() else {
            panic!("owner did not issue the browser challenge");
        };
        let signature = runtime.sign(&challenge);
        let (proof, _) = carrier.to_owner(&In::AmbientProof {
            protocol: PROTOCOL,
            admission_id: challenge.admission_id.clone(),
            body_id: challenge.body_id,
            host_id: challenge.host_id,
            boot_id: challenge.boot_id,
            nonce: challenge.nonce.to_vec(),
            signature,
        });
        let snapshot = owner
            .browser_complete(root, &authorization.window_id, proof)
            .unwrap();
        carrier.prepare_profile(&snapshot);
        let admitted = carrier.to_browser(&Out::Admitted {
            protocol: PROTOCOL,
            credential: snapshot.credential.clone(),
        });
        assert!(matches!(serde_json::from_slice::<Out>(&admitted).unwrap(),
            Out::Admitted { credential, .. } if credential == snapshot.credential));
        Some(Self {
            runtime,
            carrier,
            snapshot,
            window: authorization.window_id,
            output,
        })
    }

    pub(crate) fn finish(mut self, owner: &mut Owner, root: &Path) {
        let credential = &self.snapshot.credential;
        let request = OwnerFaceSnapshotRequest {
            schema: OWNER_FACE_REQUEST_SCHEMA.into(),
            credential_id: credential.credential_id.as_str().into(),
            body_id: credential.body_id.clone(),
            part_id: credential.part_id.clone(),
            host_id: credential.host_id.clone(),
            boot_id: credential.boot_id.clone(),
            last_seen_revision: None,
            last_seen_identity: None,
        };
        let (incoming, _) = self.carrier.to_owner(&In::FaceSnapshotRequest {
            protocol: PROTOCOL,
            request: request.clone(),
        });
        assert!(
            matches!(incoming, In::FaceSnapshotRequest { request: current, .. } if current == request)
        );
        let evidence = self
            .carrier
            .socket
            .line_evidence(&self.snapshot, &self.window)
            .unwrap();
        let binding = self.carrier.socket.binding().clone();
        let route = owner
            .browser_mask_route(&self.window, credential, &binding, Some(&evidence))
            .unwrap();
        assert!(conduit_browser_mask_offer::full_profile::selected(
            &route.planned_mask
        ));
        let face = owner.face_snapshot(&request).unwrap();
        assert_eq!(face.identity, route.face_id);
        assert_eq!(face.revision, route.face_revision);
        assert_eq!(face.basis.body_id.as_ref(), Some(&request.body_id));
        assert!(owner.browser_face_response_bytes(&request, &face).is_some());
        let raw = serde_json::to_vec(&face).unwrap();
        assert!(raw.len() > MAX_OWNER_FACE_RESPONSE_BYTES);
        let mut foreign = request.clone();
        foreign.boot_id = "boot/foreign".into();
        assert!(owner.browser_face_response_bytes(&foreign, &face).is_none());
        let frame = self.carrier.to_browser(&Out::FaceSnapshotResponse {
            protocol: PROTOCOL,
            response: OwnerFaceSnapshotResponse::Snapshot {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                interactions_admitted: owner.has_current_home_navigation(&face)
                    || crate::durable_host_control::has_remote_mask_action(&face),
                face: Box::new(face.clone()),
                route: Some(Box::new(route)),
            },
        });
        let (basis, prepared) = self.runtime.prepare(&request.body_id, &frame);
        assert_eq!(prepared["show_state"], "prepared");
        assertions::complete_view(&face, &prepared);
        let (available, show) = self.runtime.acknowledge(&prepared);
        assert_eq!(available["show_state"], "available");
        assertions::complete_view(&face, &available);
        let (canonical, decoded) = self.runtime.canonical();
        assert_eq!(
            canonical, raw,
            "canonical export must preserve every original byte"
        );
        assert_eq!(decoded, face);
        let (ack, _) = self.carrier.to_owner(&In::FaceShowAcknowledgement {
            protocol: PROTOCOL,
            request: request.clone(),
            show: Box::new(show.clone()),
        });
        let In::FaceShowAcknowledgement {
            request: ack_request,
            show: ack_show,
            ..
        } = ack
        else {
            panic!("wrong Show return");
        };
        owner
            .acknowledge_browser_mask_show(&self.window, &binding, &ack_request, &ack_show)
            .unwrap();
        self.write(
            "browser-advertisement.json",
            &serde_json::to_vec(self.snapshot.browser_advertisement.as_deref().unwrap()).unwrap(),
        );
        self.write(
            "owner-advertisement.json",
            &serde_json::to_vec(owner.host.advertisement()).unwrap(),
        );
        self.write("owner-frame.json", &frame);
        self.write("host-basis.json", &basis);
        self.write("canonical-face.json", &canonical);
        self.write(
            "available-view.json",
            &serde_json::to_vec(&available).unwrap(),
        );
        self.write("show-receipt.json", &serde_json::to_vec(&show).unwrap());
        self.write(
            "result.json",
            &serde_json::to_vec(&serde_json::json!({
                "proof_class": "native-cpu-admission-loopback-abi", "body_id": request.body_id,
                "browser_host": self.runtime.host, "browser_boot": self.runtime.boot,
                "face_id": face.identity, "face_revision": face.revision.to_string(),
                "face_bytes": raw.len(), "frame_bytes": frame.len(),
                "full_readings_per_channel": 64, "channels": 2,
                "chromium": false, "wasm": false, "physical_show": false,
            }))
            .unwrap(),
        );
        owner.browser_leave(root, &self.window, credential).unwrap();
        assert!(owner
            .acknowledge_browser_mask_show(&self.window, &binding, &request, &show)
            .is_err());
        assert!(owner.browser_face_response_bytes(&request, &face).is_none());
        eprintln!(
            "original64 authenticated native ABI proof={}",
            self.output.display()
        );
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        let path = self.output.join(name);
        assert!(!path.exists());
        fs::write(path, bytes).unwrap();
    }
}
