#![no_main]

use conduit_compile::ExactPlanDocument;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    if let Ok(plan) = serde_json::from_slice::<ExactPlanDocument>(bytes) {
        let _ = plan.validate();
    }
});
