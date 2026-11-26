//! Production C ABI exercised on native CPU, without a replacement Body or Face.
// Link the exact production implementation of the exported C ABI below.
use conduit_browser_runtime as _;
use conduit_core::{BootId, HostAdvertisement, HostId};
use conduit_face::{Face, MaskShow};
use serde_json::Value;

extern "C" {
    #[link_name = "conduit_browser_membership_input_ptr"]
    fn membership_input() -> *mut u8;
    #[link_name = "conduit_browser_membership_input_capacity"]
    fn membership_capacity() -> u32;
    #[link_name = "conduit_browser_membership_output_ptr"]
    fn membership_output() -> *const u8;
    #[link_name = "conduit_browser_membership_output_len"]
    fn membership_output_len() -> u32;
    #[link_name = "conduit_browser_membership_initialize"]
    fn initialize(host: u32, boot: u32) -> i32;
    #[link_name = "conduit_browser_membership_owner_full_profile_admit"]
    fn admit(length: u32) -> i32;
    #[link_name = "conduit_browser_membership_advertisement"]
    fn advertise() -> i32;
    #[link_name = "conduit_browser_membership_prove"]
    fn prove(length: u32) -> i32;
    #[link_name = "conduit_browser_owner_face_input_ptr"]
    fn face_input() -> usize;
    #[link_name = "conduit_browser_owner_face_input_capacity"]
    fn face_capacity() -> usize;
    #[link_name = "conduit_browser_owner_face_output_ptr"]
    fn face_output() -> usize;
    #[link_name = "conduit_browser_owner_face_output_len"]
    fn face_output_len() -> usize;
    #[link_name = "conduit_browser_owner_face_output_capacity"]
    fn face_output_capacity() -> usize;
    #[link_name = "conduit_browser_owner_face_prepare"]
    fn prepare(basis: usize, frame: usize) -> i32;
    #[link_name = "conduit_browser_owner_face_ack"]
    fn acknowledge(length: usize) -> i32;
    #[link_name = "conduit_browser_owner_face_canonical_snapshot"]
    fn canonical() -> i32;
    #[link_name = "conduit_browser_owner_face_show_receipt"]
    fn receipt() -> i32;
    #[link_name = "conduit_browser_owner_face_clear"]
    fn clear();
    #[link_name = "conduit_browser_owner_face_full_profile_ready"]
    fn full_ready() -> i32;
}

pub(super) struct Runtime {
    pub(super) host: HostId,
    pub(super) boot: BootId,
}
impl Runtime {
    pub(super) fn initialize() -> (Self, HostAdvertisement, Vec<u8>) {
        let runtime = Self {
            host: "host/browser-full64-cpu".into(),
            boot: "boot/browser-full64-cpu".into(),
        };
        unsafe { clear() };
        let mut bytes = runtime.host.as_str().as_bytes().to_vec();
        bytes.extend_from_slice(runtime.boot.as_str().as_bytes());
        bytes.extend_from_slice(&[7; 32]); // Deterministic disposable proof identity.
        put_membership(&bytes);
        assert_eq!(
            unsafe {
                initialize(
                    runtime.host.as_str().len() as u32,
                    runtime.boot.as_str().len() as u32,
                )
            },
            0
        );
        let key = get_membership();
        assert_eq!(key.len(), 32);
        let request = serde_json::to_vec(&serde_json::json!({
            "host_id": runtime.host, "boot_id": runtime.boot,
            "profile": conduit_browser_mask_offer::full_profile::PROFILE,
        }))
        .unwrap();
        put_membership(&request);
        assert_eq!(
            unsafe { admit(request.len() as u32) },
            0,
            "actual Full preparation refused"
        );
        assert_eq!(
            unsafe { advertise() },
            0,
            "actual frozen advertisement refused"
        );
        let advertisement: HostAdvertisement = serde_json::from_slice(&get_membership()).unwrap();
        assert_eq!(advertisement.host_id, runtime.host);
        assert_eq!(advertisement.boot_id, runtime.boot);
        assert!(advertisement
            .capabilities
            .contains(&conduit_browser_mask_offer::full_profile::offer()));
        assert_eq!(unsafe { full_ready() }, 1);
        (runtime, advertisement, key)
    }

    pub(super) fn sign(&self, challenge: &conduit_body::AdmissionChallenge) -> Vec<u8> {
        let bytes = serde_json::to_vec(challenge).unwrap();
        put_membership(&bytes);
        assert_eq!(unsafe { prove(bytes.len() as u32) }, 0);
        let signature = get_membership();
        assert_eq!(signature.len(), 64);
        signature
    }

    pub(super) fn prepare(&self, body: &conduit_body::BodyId, frame: &[u8]) -> (Vec<u8>, Value) {
        let basis = serde_json::to_vec(&serde_json::json!({
            "body_id": body, "host_id": self.host, "boot_id": self.boot,
            "visual_constraints": null,
        }))
        .unwrap();
        let mut input = basis.clone();
        input.extend_from_slice(frame);
        put_face(&input);
        assert_eq!(
            unsafe { prepare(basis.len(), frame.len()) },
            0,
            "actual Full ABI preparation refused"
        );
        let view = serde_json::from_slice(&get_face()).unwrap();
        (basis, view)
    }

    pub(super) fn acknowledge(&self, view: &Value) -> (Value, MaskShow) {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "show_id": view["show_id"], "face_id": view["face_id"],
            "face_revision": view["face_revision"],
        }))
        .unwrap();
        put_face(&bytes);
        assert_eq!(unsafe { acknowledge(bytes.len()) }, 0);
        let available = serde_json::from_slice(&get_face()).unwrap();
        assert_eq!(unsafe { receipt() }, 0);
        let show = serde_json::from_slice(&get_face()).unwrap();
        (available, show)
    }

    pub(super) fn canonical(&self) -> (Vec<u8>, Face) {
        assert_eq!(unsafe { canonical() }, 0);
        #[derive(serde::Deserialize)]
        struct Envelope<'a> {
            #[serde(borrow)]
            face: &'a serde_json::value::RawValue,
            face_revision_decimal: &'a str,
        }
        let bytes = get_face();
        let envelope: Envelope<'_> = serde_json::from_slice(&bytes).unwrap();
        let raw = envelope.face.get().as_bytes();
        let face: Face = serde_json::from_slice(raw).unwrap();
        assert_eq!(face.revision.to_string(), envelope.face_revision_decimal);
        (raw.to_vec(), face)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe { clear() };
    }
}

fn put_membership(bytes: &[u8]) {
    let capacity = unsafe { membership_capacity() } as usize;
    assert!(bytes.len() <= capacity);
    let pointer = unsafe { membership_input() };
    assert!(!pointer.is_null());
    // The ABI owns a stable buffer until the next call. Copy only its admitted extent.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len()) };
}
fn get_membership() -> Vec<u8> {
    let length = unsafe { membership_output_len() } as usize;
    assert!(length <= conduit_body::MAX_CANDIDATE_ADVERTISEMENT_BYTES as usize);
    let pointer = unsafe { membership_output() };
    assert!(!pointer.is_null());
    unsafe { std::slice::from_raw_parts(pointer, length) }.to_vec()
}
fn put_face(bytes: &[u8]) {
    assert!(bytes.len() <= unsafe { face_capacity() });
    let pointer = unsafe { face_input() } as *mut u8;
    assert!(!pointer.is_null());
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len()) };
}
fn get_face() -> Vec<u8> {
    let length = unsafe { face_output_len() };
    assert!(length <= unsafe { face_output_capacity() });
    let pointer = unsafe { face_output() } as *const u8;
    assert!(!pointer.is_null());
    unsafe { std::slice::from_raw_parts(pointer, length) }.to_vec()
}
