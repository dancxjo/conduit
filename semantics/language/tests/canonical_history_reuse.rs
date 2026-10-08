#[path = "common/canonical_reuse_allocation_probe.rs"]
mod allocation;
#[path = "../src/parser_canonical_history_reuse.rs"]
mod reuse;
use conduit_core::PreparedStructuredValueValidator;
use conduit_language::{parser_window8_program_bank::*, *};
use conduit_plot::rust_binding::{NativeRustBinding, PreparedNativeFamilyLimits};
struct History<'a> {
    owner: &'a u64,
    state: Vec<u8>,
    proof: Window8BankState,
}
impl reuse::CanonicalHistory<u64> for History<'_> {
    fn context_owner(&self) -> &u64 {
        self.owner
    }
    fn canonical_raw_state(&self) -> &[u8] {
        &self.state
    }
}
#[test]
#[ignore = "actual original retained Source/Native replay with explicit complete beam receipt"]
fn exact_canonical_history_reuse_requires_original_parent_replay() {
    let j: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::env::var("CANONICAL_REUSE_BEAM_RECEIPT")
                .expect("explicit original complete beam receipt"),
        )
        .unwrap(),
    )
    .unwrap();
    let s = j["receipts"][0]["epochs"][0]["beam_canonical"]
        .as_str()
        .unwrap();
    let bytes = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let beam = LanguageParserWindow8RawBeam::decode(&bytes).unwrap();
    let state = beam.candidate0().state();
    let bank = Window8ProgramBank::prepare_proposal_v2_native_evaluator(
        PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 256,
            maximum_input_bytes: 262144,
            maximum_retained_bytes: 268435456,
            maximum_preparation_peak_bytes: 536870912,
            maximum_conversion_requested_bytes: 1073741824,
        },
        Window8SourcePreparationLimits {
            maximum_retained_bytes: 536870912,
            maximum_preparation_peak_bytes: 1073741824,
            maximum_input_bytes: 262144,
        },
    )
    .unwrap();
    let context = 123u64;
    let foreign = 123u64;
    let basis = state.basis().clone().encode().unwrap();
    let validator = PreparedStructuredValueValidator::new(
        &LanguageParserWindow8RawState::semantic_type().unwrap(),
        262144,
    )
    .unwrap();
    let history = History {
        owner: &context,
        state: state.clone().encode().unwrap(),
        proof: bank.admit_state(state).unwrap(),
    };
    let replay = |h: &History<'_>| -> Result<(), ()> {
        let state = LanguageParserWindow8RawState::decode(&h.state).map_err(|_| ())?;
        let proof = bank.admit_state(&state).map_err(|_| ())?;
        assert_eq!(proof.proof(), h.proof.proof());
        Ok(())
    };
    let storage = reuse::Reuse::<u64, History<'_>>::storage();
    let (mut cache, n) = allocation::allocations(|| {
        reuse::Reuse::prepare(
            &context,
            &validator,
            &basis,
            storage.retained_inline_bytes,
            storage.construction_inline_overlap_bytes,
        )
        .unwrap()
    });
    assert_eq!(n, 0);
    assert!(reuse::Reuse::<u64, History<'_>>::prepare(
        &context,
        &validator,
        &basis,
        storage.retained_inline_bytes - 1,
        storage.construction_inline_overlap_bytes
    )
    .is_err());
    assert!(reuse::Reuse::<u64, History<'_>>::prepare(
        &context,
        &validator,
        &basis,
        storage.retained_inline_bytes,
        storage.construction_inline_overlap_bytes - 1
    )
    .is_err());
    assert_eq!(
        cache.remember(&context, &history, |_| Err("refused")),
        Err(reuse::Refusal::Replay("refused"))
    );
    assert!(cache
        .lookup(&context, &history.state, |_| Ok::<_, ()>(()))
        .unwrap()
        .is_none());
    cache.remember(&context, &history, replay).unwrap();
    assert!(core::ptr::eq(
        cache
            .lookup(&context, &history.state, replay)
            .unwrap()
            .unwrap(),
        &history
    ));
    assert!(matches!(
        cache.lookup(&foreign, &history.state, replay),
        Err(reuse::Refusal::ForeignContext)
    ));
    assert!(matches!(
        cache.lookup(&context, &history.state, |_| Err("changed-parent")),
        Err(reuse::Refusal::Replay("changed-parent"))
    ));
    assert!(cache
        .lookup(&context, &history.state[..history.state.len() - 1], replay)
        .is_err());
    let mut changed = history.state.clone();
    let offset = changed.len() - 8;
    changed[offset] = changed[offset].wrapping_add(1);
    assert!(cache.lookup(&context, &changed, replay).unwrap().is_none());
    let mut changed_basis = basis.clone();
    let last = changed_basis.len() - 1;
    changed_basis[last] ^= 1;
    let other_cache = reuse::Reuse::<u64, History<'_>>::prepare(
        &context,
        &validator,
        &changed_basis,
        storage.retained_inline_bytes,
        storage.construction_inline_overlap_bytes,
    )
    .unwrap();
    assert!(matches!(
        other_cache.lookup(&context, &history.state, replay),
        Err(reuse::Refusal::Basis)
    ));
    let (hit, n) = allocation::allocations(|| {
        cache
            .lookup(&context, &history.state, |_| Ok::<_, ()>(()))
            .unwrap()
    });
    assert!(hit.is_some());
    assert_eq!(n, 0);
    println!("PASS borrowed four-parent fullcanonical key, exactowner/Type/basis, original fullNative+Source replay, mandatoryreplay refusal, malformed, inline oneunder, zeroallocmechanical lookup; {storage:?}; histories/validator/replay allocation separately charged, no admission authority");
}
