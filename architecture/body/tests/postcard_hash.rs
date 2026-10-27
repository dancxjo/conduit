//! Compatibility oracle for the two historical Body postcard hash flavors.
#[path = "../../core/tests/common/sealed_state.rs"]
mod core_fixture;
#[path = "../src/postcard_hash.rs"]
mod postcard_hash;

use conduit_body::{Body, BodyBiographyEvidence, BodyMembership};
use serde::Serialize;
use sha2::{Digest, Sha256};

struct OldArchive(Sha256);
struct OldState(Sha256);
impl postcard::ser_flavors::Flavor for OldArchive {
    type Output = [u8; 32];
    fn try_push(&mut self, byte: u8) -> postcard::Result<()> {
        self.0.update([byte]);
        Ok(())
    }
    fn try_extend(&mut self, bytes: &[u8]) -> postcard::Result<()> {
        self.0.update(bytes);
        Ok(())
    }
    fn finalize(self) -> postcard::Result<Self::Output> {
        Ok(self.0.finalize().into())
    }
}
impl postcard::ser_flavors::Flavor for OldState {
    type Output = [u8; 32];
    fn try_push(&mut self, byte: u8) -> postcard::Result<()> {
        self.0.update([byte]);
        Ok(())
    }
    fn finalize(self) -> postcard::Result<Self::Output> {
        Ok(self.0.finalize().into())
    }
}

fn check<T: Serialize>(value: &T) {
    let bytes = postcard::to_allocvec(value).unwrap();
    for prefix in [b"".as_slice(), b"conduit.body/prepared-proposal-state@1"] {
        let mut seed = Sha256::new();
        seed.update(prefix);
        let mut oracle = seed.clone();
        oracle.update(&bytes);
        let expected: [u8; 32] = oracle.finalize().into();
        assert_eq!(
            postcard::serialize_with_flavor(value, OldArchive(seed.clone())).unwrap(),
            expected
        );
        assert_eq!(
            postcard::serialize_with_flavor(value, OldState(seed.clone())).unwrap(),
            expected
        );
        assert_eq!(
            postcard::serialize_with_flavor(value, postcard_hash::PostcardSha256(seed)).unwrap(),
            expected
        );
    }
}

#[test]
fn full_plan_and_biography_match_historical_flavors_and_serialized_bytes() {
    let plan = core_fixture::seal(core_fixture::fragment());
    let body = Body::born(
        "source/work".into(),
        "checked/work".into(),
        1,
        "born".into(),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence = BodyBiographyEvidence::born(body, membership, "Test".into()).unwrap();
    check(&plan);
    check(&evidence);
    check(&(&evidence, Some(&plan), &[&plan]));
}

#[test]
fn byte_boundaries_and_domain_separation_remain_exact() {
    for size in [0, 1, 63, 64, 65, 127, 128, 129, 1024] {
        check(&(0..size).map(|i| i as u8).collect::<Vec<_>>());
    }
    let plain =
        postcard::serialize_with_flavor(&42u64, postcard_hash::PostcardSha256(Sha256::new()))
            .unwrap();
    let mut domain = Sha256::new();
    domain.update(b"conduit.body/prepared-proposal-state@1");
    let prefixed =
        postcard::serialize_with_flavor(&42u64, postcard_hash::PostcardSha256(domain)).unwrap();
    assert_ne!(plain, prefixed);
}
