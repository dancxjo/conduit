//! Shared full-frame fixture custody and codecs; no registered tests.
//! Full conditioner plus four recurrent subframes through one ordinary Plan.
use super::{conditioning, pinned, plan::*, runtime::*, shared::*, state::*};
use std::{collections::BTreeMap, path::Path};
pub const ENTRY: &str = "speech/fargan-frame";
pub fn schema() -> SourceSchema {
    SourceSchema::for_entry(&source(), ENTRY)
}
pub fn resources(root: &Path, schema: &SourceSchema) -> Resources {
    let signal = SourceSchema::prepare();
    let condition = SourceSchema::for_entry(&source(), "speech/fargan-conditioning");
    let resources: Resources = pinned::load(root, &signal)
        .into_iter()
        .map(|(n, r)| (format!("signal_{n}"), r))
        .chain(
            pinned::conditioning_resources(root, &condition)
                .into_iter()
                .map(|(n, r)| (format!("cond_{n}"), r)),
        )
        .collect();
    for (name, resource) in &resources {
        assert_eq!(&resource.value_type, schema.ty(name));
    }
    assert_eq!(resources.len(), schema.resource_shapes.len());
    resources
}
pub fn inputs(
    schema: &SourceSchema,
    features: &[f32; 20],
    history: &[f32; 128],
    period: u16,
    previous: u16,
    state: &State,
) -> BTreeMap<String, Vec<u8>> {
    let mut inputs = conditioning::input(schema, features, history, period, previous);
    inputs.insert("state".into(), state.encode(schema.ty("state")));
    inputs
}
pub fn pcm(result: &EncodedResultAndTiming) -> Vec<f32> {
    let mut pcm = Vec::with_capacity(160);
    for name in ["pcm0", "pcm1", "pcm2"] {
        pcm.extend(floats(&result.outputs[name]));
    }
    pcm.extend(floats(field(&result.value, "pcm_f32")));
    assert_eq!(pcm.len(), 160);
    pcm
}
pub struct Case {
    pub features: [f32; 20],
    pub pcm: [f32; 160],
    pub next: State,
    pub history: [f32; 128],
    pub period: u16,
}
pub struct Oracle {
    pub state: State,
    pub history: [f32; 128],
    pub period: u16,
    pub cases: Vec<Case>,
}
fn f32s(bytes: &[u8]) -> Vec<f32> {
    let v: Vec<_> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v))
        .collect();
    assert!(v.iter().all(|v| v.is_finite()));
    v
}
fn period(bytes: &[u8]) -> u16 {
    let p = u16::try_from(i32::from_le_bytes(bytes.try_into().unwrap())).unwrap();
    assert!((32..=255).contains(&p));
    p
}
pub fn oracle(root: &Path) -> Oracle {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(root.join("full-frame-oracle.bin")).unwrap();
    assert_eq!(bytes.len(), 3864 + 24 * 4584);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "9c676f6c76aec71281ea097fe0c477c5a3b77f5022e639a0d82dbb4a5437fcc3"
    );
    Oracle {
        state: State::from_oracle(&f32s(&bytes[..3348])),
        history: f32s(&bytes[3348..3860]).try_into().unwrap(),
        period: period(&bytes[3860..3864]),
        cases: bytes[3864..]
            .as_chunks::<4584>()
            .0
            .iter()
            .map(|r| Case {
                features: f32s(&r[..80]).try_into().unwrap(),
                pcm: f32s(&r[80..720]).try_into().unwrap(),
                next: State::from_oracle(&f32s(&r[720..4068])),
                history: f32s(&r[4068..4580]).try_into().unwrap(),
                period: period(&r[4580..]),
            })
            .collect(),
    }
}
