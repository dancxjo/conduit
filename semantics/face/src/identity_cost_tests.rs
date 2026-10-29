//! Opt-in CPU attribution of an independently retained publication fixture.
extern crate std;
use super::*;

#[test]
#[ignore = "requires a privately retained actual CPU publication fixture"]
fn retained_publication_face_validation_cost() {
    let path = std::env::var_os("CONDUIT_PUBLICATION_FACE").expect("fixture path");
    let bytes = std::fs::read(path).unwrap();
    let face: Face = serde_json::from_slice(&bytes).unwrap();
    face.validate().unwrap();
    std::eprintln!("face_cpu fixture_sha256={:x}", Sha256::digest(&bytes));
    for _ in 0..5 {
        let clock = std::time::Instant::now();
        for _ in 0..100 {
            core::hint::black_box(&face).validate_content().unwrap();
        }
        let content_ns = clock.elapsed().as_nanos() / 100;
        let clock = std::time::Instant::now();
        for _ in 0..100 {
            core::hint::black_box(core::hint::black_box(&face).content_digest_bytes());
        }
        let digest_ns = clock.elapsed().as_nanos() / 100;
        std::eprintln!("face_cpu content_ns={content_ns} digest_ns={digest_ns}");
    }
}

#[path = "identity_cost_oracle.rs"]
mod frozen;

#[test]
#[ignore = "requires privately retained actual CPU publication fixture and export directory"]
fn retained_publication_canonical_chunks() {
    let bytes = std::fs::read(std::env::var_os("CONDUIT_PUBLICATION_FACE").unwrap()).unwrap();
    let face: Face = serde_json::from_slice(&bytes).unwrap();
    let (actual, chunks) = frozen::capture(&face);
    assert_eq!(actual, face.content_digest_bytes());
    // Raw transcript parity is independent of whether these mutated values
    // would be admitted as a current Face. No validation witness is fabricated.
    let mut edge = face.clone();
    edge.revision = u64::MAX;
    edge.subjects[0].name = "α\n🦀".into();
    let subject = edge.subjects[0].identity.clone();
    for (name, value) in [
        (
            "diagnostic/count",
            crate::FacePropertyValue::Count(u64::MAX),
        ),
        (
            "diagnostic/signed",
            crate::FacePropertyValue::Signed(i64::MIN),
        ),
        (
            "diagnostic/empty",
            crate::FacePropertyValue::Text("".into()),
        ),
        (
            "diagnostic/binary",
            crate::FacePropertyValue::Content(alloc::vec![0, 0, 0, 0, 0, 0, 0xf8, 0x7f]),
        ),
    ] {
        edge.properties.push(crate::FaceProperty {
            subject: subject.clone(),
            name: name.into(),
            value,
        });
    }
    assert_eq!(frozen::capture(&edge).0, edge.content_digest_bytes());
    let transcript: alloc::vec::Vec<u8> = chunks.iter().flatten().copied().collect();
    let lengths: alloc::vec::Vec<usize> = chunks.iter().map(|chunk| chunk.len()).collect();
    let directory =
        std::path::PathBuf::from(std::env::var_os("CONDUIT_PUBLICATION_DIAGNOSTIC_DIR").unwrap());
    std::fs::write(directory.join("canonical-transcript.bin"), &transcript).unwrap();
    std::fs::write(
        directory.join("canonical-chunk-lengths.json"),
        serde_json::to_vec(&lengths).unwrap(),
    )
    .unwrap();
    std::eprintln!(
        "face_chunks calls={} bytes={} digest={:x}",
        chunks.len(),
        transcript.len(),
        Sha256::digest(&transcript)
    );
}
