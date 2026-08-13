//! Real browser-Host admission into the public front-door Body.

use super::{PatchbayHtmlServer, ServerError, MAX_BROWSER_WASM_BYTES};
use crate::front_door::snapshot_for_front_door;
use conduit_body::{AdmissionSigns, AmbientAdmissionProof, CandidateObservation, DiscoveryProofId};
use conduit_core::{
    process_owned_line_offer_with_limits, ConnectionBase, LineAvailability, LineId, LinkBindingId,
    LinkLimits, SignId,
};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BROWSER_ADMISSION_PROTOCOL, MAX_BROWSER_ADMISSION_FRAME_BYTES,
};
use patchbay_model::LocalFrontDoor;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const BROWSER_LINE_ID: &str = "patchbay-html/browser-admission-line";
const BROWSER_BINDING_ID: &str = "patchbay-html/browser-admission-binding";

impl PatchbayHtmlServer {
    pub fn bind_browser_front_door_ephemeral() -> Result<Self, ServerError> {
        let session = Arc::new(Mutex::new(
            LocalFrontDoor::fresh().map_err(ServerError::Interaction)?,
        ));
        let snapshot = {
            let session = session
                .lock()
                .map_err(|_| ServerError::Interaction("front-door session lock failed".into()))?;
            snapshot_for_front_door(&session).map_err(ServerError::Interaction)?
        };
        let listener = BrowserAdmissionListener::bind_loopback()
            .map_err(|error| ServerError::Interaction(format!("Body admission Line: {error:?}")))?;
        let body_url = listener
            .url()
            .map_err(|error| ServerError::Interaction(format!("Body admission URL: {error:?}")))?;
        let wasm_path = std::env::var_os("CONDUIT_BROWSER_RUNTIME_WASM")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from("target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm")
            });
        let metadata = std::fs::metadata(&wasm_path).map_err(|error| {
            ServerError::Interaction(format!(
                "browser Host runtime {} is unavailable ({error}); run through `cargo xtask demo patchbay --on browser`",
                wasm_path.display()
            ))
        })?;
        if !metadata.is_file() || metadata.len() > MAX_BROWSER_WASM_BYTES as u64 {
            return Err(ServerError::Interaction(
                "browser Host runtime is not one bounded regular WASM artifact".into(),
            ));
        }
        let browser_wasm = std::fs::read(&wasm_path).map_err(|error| {
            ServerError::Interaction(format!("cannot read browser Host runtime: {error}"))
        })?;
        let mut server = Self::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(), &snapshot)?;
        server.front_door = Some(Arc::clone(&session));
        server.body_admission = Some(
            serde_json::to_vec(&serde_json::json!({ "url": body_url.clone() }))
                .map_err(|error| ServerError::Interaction(error.to_string()))?,
        );
        server.browser_wasm = Some(browser_wasm);
        std::thread::Builder::new()
            .name("patchbay-browser-admission".into())
            .spawn(move || {
                if let Err(error) = admit_one_browser(listener, session, body_url) {
                    eprintln!("Patchbay browser admission refused: {error}");
                }
            })
            .map_err(ServerError::Io)?;
        Ok(server)
    }
}

fn admit_one_browser(
    listener: BrowserAdmissionListener,
    session: Arc<Mutex<LocalFrontDoor>>,
    base_instance_id: String,
) -> Result<(), String> {
    let mut socket = listener.accept().map_err(debug("accept browser"))?;
    let (frame, encoded_bytes) = socket
        .receive_with_size()
        .map_err(debug("browser advertisement"))?;
    let BrowserAdmissionIngress::Advertise {
        advertisement,
        friendly_label,
        verifying_key,
        freshness_sequence,
        ..
    } = frame
    else {
        return Err("browser did not begin with an advertisement".into());
    };
    let mut front_door = session
        .lock()
        .map_err(|_| "front-door session lock failed")?;
    let candidate_id = front_door.observe_candidate(CandidateObservation {
        advertisement: advertisement.clone(),
        friendly_label,
        observed_binding_id: LinkBindingId::from(BROWSER_BINDING_ID),
        observation_sign_id: SignId::from("patchbay-html/browser-observed"),
        proof_id: DiscoveryProofId::bind("patchbay-html/browser-discovery")
            .map_err(debug("browser discovery proof"))?,
        freshness_sequence,
        encoded_bytes,
    })?;
    let line = process_owned_line_offer_with_limits(
        BROWSER_LINE_ID,
        BROWSER_BINDING_ID,
        ConnectionBase::WebSocket,
        &base_instance_id,
        &advertisement,
        front_door.advertisement(),
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
            maximum_buffered_bytes: (MAX_BROWSER_ADMISSION_FRAME_BYTES * 2) as u32,
            maximum_frame_bytes: MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
        },
    );
    front_door.observe_line(line)?;
    drop(front_door);
    let challenge = session
        .lock()
        .map_err(|_| "front-door session lock failed")?
        .begin_ambient_admission(
            &candidate_id,
            verifying_key
                .try_into()
                .map_err(|_| "browser verifying key has the wrong length")?,
            [43; 32],
            1_000,
            60_000,
            SignId::from("patchbay-html/browser-admission-requested"),
        )?;
    socket
        .send(&BrowserAdmissionEgress::Challenge {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            challenge,
        })
        .map_err(debug("send browser challenge"))?;
    let proof = ambient_proof(socket.receive().map_err(debug("browser proof"))?)?;
    let credential = session
        .lock()
        .map_err(|_| "front-door session lock failed")?
        .complete_ambient_admission(
            &proof,
            1_001,
            AdmissionSigns {
                part_admitted: SignId::from("patchbay-html/browser-part-admitted"),
                host_attached: SignId::from("patchbay-html/browser-host-attached"),
                candidate_admitted: SignId::from("patchbay-html/browser-candidate-admitted"),
            },
        )?;
    socket
        .send(&BrowserAdmissionEgress::Admitted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential: credential.clone(),
        })
        .map_err(debug("send browser admission"))?;
    if socket.receive().is_ok() {
        return Err("admitted browser sent an unexpected extra admission frame".into());
    }
    let mut front_door = session
        .lock()
        .map_err(|_| "front-door session lock failed")?;
    front_door.observe_line_availability(
        &LineId::from(BROWSER_LINE_ID),
        LineAvailability::Unavailable,
        SignId::from("patchbay-html/browser-line-unavailable"),
    )?;
    front_door.observe_part_offline(
        &credential.part_id,
        &advertisement.boot_id,
        SignId::from("patchbay-html/browser-offline"),
    )
}

fn ambient_proof(frame: BrowserAdmissionIngress) -> Result<AmbientAdmissionProof, String> {
    match frame {
        BrowserAdmissionIngress::AmbientProof {
            admission_id,
            body_id,
            host_id,
            boot_id,
            nonce,
            signature,
            ..
        } => Ok(AmbientAdmissionProof {
            admission_id,
            body_id,
            host_id,
            boot_id,
            nonce: nonce.try_into().map_err(|_| "invalid browser nonce")?,
            signature: signature
                .try_into()
                .map_err(|_| "invalid browser signature")?,
        }),
        _ => Err("browser did not answer with ambient proof".into()),
    }
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
