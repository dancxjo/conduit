use super::*;
use crate::{flow_activation::required_spoken_template_offers, StdHostConfig};

#[test]
fn frame_boundaries_preserve_unicode_whitespace_and_words_exactly() {
    assert!(frame_ranges(" \t\n").is_err());
    for script in [
        "Milk. Eggs. Bread.".to_string(),
        "naïve λογική 水 ".repeat(80),
        format!("Short prefix {} remaining.", "a".repeat(76)),
        "word ".repeat(1200),
    ] {
        let ranges = frame_ranges(&script).unwrap();
        assert!(ranges.len() <= MAX_SPOKEN_TEMPLATE_FRAMES);
        let joined: String = ranges.iter().map(|range| &script[range.clone()]).collect();
        assert_eq!(joined, script);
        assert_eq!(
            Sha256::digest(joined.as_bytes()),
            Sha256::digest(script.as_bytes())
        );
        for range in ranges {
            assert!(!range.is_empty());
            assert!(range.len() <= MAX_SPOKEN_TEMPLATE_FRAME_BYTES);
            assert!(word_boundary(&script, range.start));
            assert!(word_boundary(&script, range.end));
        }
    }
    assert!(frame_ranges("").is_err());
    assert!(frame_ranges(&" ".repeat(8192)).is_err());
    assert!(frame_ranges(&"a".repeat(MAX_SPOKEN_TEMPLATE_FRAME_BYTES + 1)).is_err());
    assert!(frame_ranges(&"x ".repeat(conduit_face::MAX_SPOKEN_TEMPLATE_BYTES)).is_err());
}

fn host(boot: &str) -> StdHost {
    let baseline = StdHost::new_with_config(StdHostConfig {
        host_id: "proof/spoken-template-reading".into(),
        boot_id: boot.into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let mut advertisement = baseline.advertisement().clone();
    advertisement
        .capabilities
        .extend(required_spoken_template_offers().unwrap());
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    StdHost::from_advertisement(advertisement).unwrap()
}

#[test]
#[ignore = "requires explicit actual retained Todo Face; no manufactured Body or template grant"]
fn actual_face_reading_is_source_backed_complete_and_stale_safe() {
    let path = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_SPOKEN_TEMPLATE_FACE").expect("actual retained Face path"),
    );
    assert!(path.is_absolute());
    let record: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let face: Face = serde_json::from_value(record.get("face").unwrap_or(&record).clone()).unwrap();
    assert!(face.basis.body_id.is_some());
    let host = host("proof/spoken-template-reading/boot");
    let mut receipts = Vec::new();
    for entry in ["spoken/brief-template", "spoken/expanded-template"] {
        let prepared = PreparedSpokenTemplateReading::prepare(&host, &face, entry).unwrap();
        prepared.validate_current(&host, &face).unwrap();
        assert_eq!(
            prepared.opening_wording(),
            prepared.framed_wording(0).unwrap()
        );
        let joined: String = (0..prepared.frames_count())
            .map(|index| prepared.framed_wording(index).unwrap())
            .collect();
        assert_eq!(joined, prepared.receipt().wording);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(joined.as_bytes())),
            prepared.receipt().wording_digest
        );
        assert!(prepared.framed_wording(prepared.frames_count()).is_err());
        let mut stale = face.clone();
        stale.revision += 1;
        assert!(prepared.validate_current(&host, &stale).is_err());
        let mut changed = host.advertisement().clone();
        changed.offer_generation.0 += 1;
        let changed = StdHost::from_advertisement(changed).unwrap();
        assert!(prepared.validate_current(&changed, &face).is_err());
        let mut changed = host.advertisement().clone();
        changed.boot_id = "proof/replaced-boot".into();
        let changed = StdHost::from_advertisement(changed).unwrap();
        assert!(prepared.validate_current(&changed, &face).is_err());
        receipts.push(prepared.receipt().clone());
    }
    assert_ne!(receipts[0].wording, receipts[1].wording);
    assert_eq!(
        receipts[0].selected_clause_indices,
        receipts[1].selected_clause_indices
    );
    assert!(
        PreparedSpokenTemplateReading::prepare(&host, &face, "spoken/invented-template").is_err()
    );
    assert_eq!(receipts[0].source_face_identity, face.identity.as_str());
}
