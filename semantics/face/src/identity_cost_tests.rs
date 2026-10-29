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
