//! Captured Parsec Face/component data cannot supply absent speech equipment.
use conduit_core::OfferGeneration;
use conduit_face::Face;
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};

#[test]
#[ignore = "requires validated original-frame records in CONDUIT_PARSEC_BROWSER_RECORDS"]
fn captured_parsec_readings_cannot_plan_on_a_host_without_speech_offers() {
    let path = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_PARSEC_BROWSER_RECORDS").expect("explicit captured records"),
    );
    assert!(path.is_absolute());
    let records: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        records["schema"],
        "conduit.proof/parsec-component-reconstruction@1"
    );
    let records = records["records"].as_array().unwrap();
    assert_eq!(records.len(), 4);
    let host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "host/parsec-no-speech".into(),
            boot_id: "boot/parsec-no-speech".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    let original_host = host.advertisement().clone();
    for record in records {
        let face: Face = serde_json::from_value(record["face"].clone()).unwrap();
        face.validate().unwrap();
        assert_eq!(record["face_revision_decimal"], face.revision.to_string());
        assert_eq!(record["provenance"]["action"], face.actions[0].identity);
        assert!(record["wording"].as_str().unwrap().starts_with("Input: "));
        // Wording and captured provenance remain documentary. Ordinary Host
        // preparation must refuse before any Play, artifact creation or Show.
        let refusal = match host.prepare_direct_spoken_mask(&face) {
            Ok(_) => panic!("captured wording invented absent speech equipment"),
            Err(refusal) => refusal,
        };
        assert!(
            refusal.starts_with("place direct spoken Mask:"),
            "unexpected refusal: {refusal}"
        );
        eprintln!("absent speech Host refuses before Plan: {refusal}");
        assert_eq!(host.advertisement(), &original_host);
    }
}
