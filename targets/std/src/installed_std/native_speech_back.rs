//! Admission and fixed-storage preparation for the compiled native voice.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{HostedValueStore, PortId};
use conduit_speech::kernel::{NativeSpeechBack, IMPLEMENTATION, MAXIMUM_PCM_BYTES};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};
fn admitted(placement: &PlannedGear) -> Result<NativeSpeechBack, String> {
    NativeSpeechBack::prepare::<{ super::PORTS }>(placement, PortId(0), PortId(0))
        .map_err(|error| format!("native speech preparation: {error:?}"))
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    // Validate exact realization before reserving the profile's ceilings.
    let _ = admitted(placement)?;
    let blocks = conduit_speech::MAXIMUM_UTTERANCE_FRAMES
        .div_ceil(conduit_speech::MAXIMUM_BLOCK_FRAMES as u64);
    let signs = blocks
        .checked_mul(16)
        .and_then(|n| n.checked_add(16))
        .and_then(|n| u16::try_from(n).ok())
        .ok_or("native speech Sign budget exceeds installed profile")?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: MAXIMUM_PCM_BYTES as u32,
        host_requests: 0,
        sign_items: signs,
        maximum_value_bytes: MAXIMUM_PCM_BYTES as u32,
    })
}
fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<InstalledBack, String> {
    Ok(InstalledBack::NativeSpeech(Box::new(admitted(placement)?)))
}
