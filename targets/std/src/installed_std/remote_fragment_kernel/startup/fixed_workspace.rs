//! Constructor-owned fixed terms for the closed startup recipe. This excludes
//! lowering, ledger staging, metadata/provider clones, caller captures and
//! errors, so it is not a complete preparation receipt or execution authority.
use crate::installed_std::{
    kernel_preparation::KernelTables, InstalledBack, InstalledScheduler, MAX_NODES,
};
use conduit_kernel::{HostedSignLog, HostedValueStore};
use core::mem::size_of;

pub(super) struct FixedWorkspace {
    pub(super) values: usize,
    pub(super) signs: usize,
    pub(super) retained_kernel: usize,
    pub(super) constructor_requests: usize,
    pub(super) expected_offer_requests: Option<usize>,
}
pub(super) fn derive() -> Option<FixedWorkspace> {
    if !cfg!(all(
        target_pointer_width = "64",
        conduit_pinned_collection_layout
    )) {
        return None;
    }
    // Remote preparation starts maximum_value_bytes at TICK_ENCODED_LEN;
    // source budget then contributes exactly one Bool slot, sink no slots.
    let maximum = crate::installed_std::TICK_ENCODED_LEN.max(conduit_core::BOOL_ENCODED_LEN as u32);
    let values = HostedValueStore::canonical_storage_bytes(core::iter::once(maximum)).ok()?;
    // prepare_with_retention default32 + body/wake8 + startup-chime16;
    // no remote endpoints and default retention contribute zero remote rows.
    let signs = HostedSignLog::canonical_storage_bytes(32 + 8 + 16, 0)?;
    let retained_kernel = size_of::<InstalledScheduler>()
        .checked_add(values)?
        .checked_add(signs)?
        .checked_add(crate::hosted_audio::startup_chime::PCM_BYTES)?
        // Same qualified Arc<AtomicU8> layout as back_capacity.rs.
        .checked_add(size_of::<[usize; 3]>())?;
    // preparation.rs first collects the MAX_NODES operation Vec, then consumes
    // it through another collect before array conversion. Count both exact-size
    // buffers even when the pinned library reuses the original allocation.
    // KernelTables and the new boxed scheduler overlap during install.
    let drivers = MAX_NODES.checked_mul(size_of::<InstalledBack>())?;
    let auxiliary_requests = auxiliary_requests()?;
    let constructor_requests = retained_kernel
        .checked_add(size_of::<KernelTables>())?
        .checked_add(drivers)?
        .checked_add(drivers)?
        .checked_add(auxiliary_requests)?;
    Some(FixedWorkspace {
        values,
        signs,
        retained_kernel,
        constructor_requests,
        expected_offer_requests: super::offer_workspace::derive(),
    })
}

// All six optional-provider tables are allocated even though the exact closed
// source/cue fragment fills them only with None. These are requested heap bytes,
// not inline InstalledRemoteFragment fields or any provider payload.
fn auxiliary_requests() -> Option<usize> {
    use crate::installed_std::{
        body_chat_prompt_back::BodyChatPromptHost,
        generated_speech_commit_back::GeneratedSpeechCommitHost,
        recognized_turn_commit_back::RecognizedTurnCommitHost,
        speech_recognition_adapter_back::{SpeechResultToEventStreamHost, SpeechWindowToClipHost},
        whisper_language::PreparedWhisperLanguage,
    };
    // WhisperLanguages::prepare and the four ordinary prepare_*_hosts helpers
    // collect trusted-length maps, requesting exactly two slots each.
    let mut bytes = 2usize.checked_mul(size_of::<Option<PreparedWhisperLanguage>>())?;
    bytes =
        bytes.checked_add(2usize.checked_mul(size_of::<Option<RecognizedTurnCommitHost>>())?)?;
    bytes = bytes.checked_add(2usize.checked_mul(size_of::<Option<SpeechWindowToClipHost>>())?)?;
    bytes = bytes
        .checked_add(2usize.checked_mul(size_of::<Option<SpeechResultToEventStreamHost>>())?)?;
    bytes = bytes.checked_add(2usize.checked_mul(size_of::<Option<BodyChatPromptHost>>())?)?;
    // GeneratedSpeechCommitHost::prepare_hosts collects Result<Vec<_>>. Its
    // GenericShunt lower hint is zero. Charge every pinned RawVec capacity
    // epoch for two None rows, including old/new realloc-copy overlap.
    bytes = bytes.checked_add(two_result_rows::<Option<GeneratedSpeechCommitHost>>()?)?;
    // DataTextGenerationHosts::prepare always constructs this owned KindId,
    // even when no data-Text Back exists. Empty stores/bindings allocate zero.
    bytes = bytes.checked_add("value/text".len())?;
    // prepare_startup_outputs try_reserve_exact(count), with exactly one cue.
    // Provider argument/selection backing is a separate constructor term.
    bytes.checked_add(size_of::<super::RemoteCue>())
}

fn two_result_rows<T>() -> Option<usize> {
    let size = size_of::<T>();
    if size == 0 {
        return Some(0);
    }
    // Qualified alloc/raw_vec/mod.rs:158,503: mincap8 for bytes,4 for
    // sizes2..1024,1 otherwise; growth=max(2*old,len+additional,mincap).
    let minimum: usize = if size == 1 {
        8
    } else if size <= 1024 {
        4
    } else {
        1
    };
    let first = minimum.checked_mul(size)?;
    if minimum >= 2 {
        Some(first)
    } else {
        first.checked_add(2usize.checked_mul(size)?)
    }
}
