//! Optional private PUBLIC-wire capture calibration. No device/key material is
//! required. Output contains only byte counts, allocation counts and a digest.
use conduit_body::*;
use serde::de::DeserializeSeed;
use sha2::{Digest, Sha256};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

#[test]
#[ignore = "requires CONDUIT_PUBLIC_RETURN_RECEIPT pointing to public signed wire evidence"]
fn captured_projection_phase_has_no_transcript_or_second_projection_owner() {
    let path = std::env::var_os("CONDUIT_PUBLIC_RETURN_RECEIPT").expect("public receipt path");
    let outer = std::fs::read(path).unwrap();
    let signed: SignedPortableAdmissionReceipt = serde_json::from_slice(&outer).unwrap();
    let ack: MemberReturnImageAcknowledgement = serde_json::from_str(&signed.receipt_json).unwrap();
    signed
        .transport_metadata()
        .unwrap()
        .verify(
            signed.receipt_json.as_bytes(),
            &signed.owner,
            &signed.member_verifying_key,
        )
        .unwrap();
    let (context_copy, context_cost) = probe::observe(|| ack.projection.context.clone());
    drop(context_copy);
    let (grant_copy, grant_cost) = probe::observe(|| ack.projection.authority_grants.clone());
    drop(grant_copy);
    let reference = member_return_image_transcript(&ack.credential, &ack.projection).unwrap();
    // Existing current Root and original Issuer descriptions are baseline
    // owners, outside the transaction observation. Include callback grant copies,
    // image hashes, caller document backing, transcript and semantic ack decode.
    let ((credential, (input_extent, presentation_max)), cost) = probe::observe(|| {
        let mut input = vec![0u8; 9 * 1024];
        let hashes = ack.projection.capability_images.clone();
        let grants = ack.projection.authority_grants.clone();
        let actual = BorrowedHostAdvertisementImageProjection {
            context: &ack.projection.context,
            capability_images: &hashes,
            authority_grants: &grants,
        };
        assert_eq!(
            member_return_image_transcript_into(&ack.credential, actual, &mut input).unwrap(),
            reference
        );
        #[derive(serde::Serialize)]
        struct Presentation<'a> {
            kind: &'static str,
            schema: &'static str,
            credential: &'a MembershipCredential,
            projection: BorrowedHostAdvertisementImageProjection<'a>,
            signature: &'a [u8],
        }
        // Every signature octet at255 gives the maximum existing decimal-array
        // signature carriage length without reading or generating any secret.
        let presentation = Presentation {
            kind: "Advertise",
            schema: MEMBER_RETURN_IMAGES_SCHEMA,
            credential: &ack.credential,
            projection: actual,
            signature: &[255; 64],
        };
        let presentation_max = serde_json_core::to_slice(&presentation, &mut input).unwrap();
        assert!(signed.receipt_json.len() <= input.len());
        input[..signed.receipt_json.len()].copy_from_slice(signed.receipt_json.as_bytes());
        let mut decoder =
            serde_json_core::de::Deserializer::new(&input[..signed.receipt_json.len()], None);
        let credential = MemberReturnImageAcknowledgementSeed {
            retained: &ack.credential,
            actual,
        }
        .deserialize(&mut decoder)
        .unwrap();
        assert_eq!(decoder.end().unwrap(), signed.receipt_json.len());
        (credential, (input.len(), presentation_max))
    });
    assert_eq!(credential, ack.credential);
    println!("projection-phase outer={} ack={} input={} presentation_max={} images={} grants={} peak_dynamic={} context_copy={} grant_copy={} allocations={} sha256={:x}",
        outer.len(), signed.receipt_json.len(), input_extent, presentation_max, ack.projection.capability_images.len(), ack.projection.authority_grants.len(), cost.peak_bytes, context_cost.peak_bytes, grant_cost.peak_bytes, cost.allocations, Sha256::digest(&outer));
    // This is an explicitly labelled phase measurement, not the whole signed
    // carrier transaction or a 32-bit MCU admission claim.
}
