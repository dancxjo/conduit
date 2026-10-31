//! Fresh software preparation must preserve actual storage on refusal.
use conduit_environment_make::NativeSoftwareSourceRecipe;
use conduit_environment_runtime::{
    appliance::assigned::{NativePreparationRefusal, PreparedResidentSoftware},
    construction::compile,
};
use conduit_kernel::{FixedValueStore, ValueStorage};

#[test]
fn checked_source_prepares_fresh_backs_and_full_store_refusal_is_recoverable() {
    let source = compile(960, 60_000).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let spec = recipe.spec();
    let texts = recipe.resident_computations().collect::<Vec<_>>();
    let mut full = FixedValueStore::<1, 64>::new(64).unwrap();
    let stale = full.store(b"old").unwrap();
    full.release(stale).unwrap();
    let live = full.store(&[42; 64]).unwrap();
    let mut returned = match PreparedResidentSoftware::prepare_source_retained(&spec, &texts, full)
    {
        Ok(_) => panic!("occupied storage cannot admit a wait"),
        Err((NativePreparationRefusal::Storage(_), values)) => values,
        Err(_) => panic!("valid checked codecs must reach storage admission"),
    };
    assert_eq!(returned.get(live).unwrap(), &[42; 64]);
    assert!(returned.get(stale).is_err());
    assert_eq!(returned.used_items(), 1);
    returned.release(live).unwrap();
    let prepared = match PreparedResidentSoftware::prepare_source_retained(&spec, &texts, returned)
    {
        Ok(prepared) => prepared,
        Err(_) => panic!("released actual store should admit fresh software"),
    };
    assert!(prepared
        .source_offers(&spec, &recipe.resident_offers())
        .is_ok());
}
