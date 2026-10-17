//! Language requests retained per exact prepared recognition gear, before play.
use conduit_core::PlanFragment;
use conduit_language::LanguageRequest;

pub(super) struct WhisperLanguages(Vec<Option<PreparedWhisperLanguage>>);

pub(super) struct PreparedWhisperLanguage {
    request: LanguageRequest,
    coverage: conduit_language::LanguageCoverage,
}

impl WhisperLanguages {
    pub(super) fn prepare(
        fragment: &PlanFragment,
        identity: &conduit_plan_lowering::lowering::KernelIdentityMap,
    ) -> Result<Self, String> {
        if fragment.placements.len() > super::MAX_NODES
            || identity.placements.len() != fragment.placements.len()
        {
            return Err(
                "recognition Language preparation exceeds the admitted node profile".into(),
            );
        }
        let mut prepared = (0..identity.placements.len())
            .map(|_| None)
            .collect::<Vec<_>>();
        for placement in &fragment.placements {
            if !matches!(
                placement.kind_id.as_str(),
                conduit_tongues::SPEECH_RECOGNIZE_KIND
                    | conduit_tongues::SPEECH_RECOGNIZE_CLIP_KIND
            ) {
                continue;
            }
            let node = identity
                .node_for_placement(&placement.placement_id)
                .ok_or("recognition placement has no exact lowered node")?;
            let slot = prepared
                .get_mut(usize::from(node.0))
                .ok_or("recognition node exceeds the prepared Language table")?;
            if slot.is_some() {
                return Err("recognition Language node is duplicated".into());
            }
            let request = crate::hosted_language::admit(placement)
                .map_err(|error| format!("recognition Language preparation: {error:?}"))?;
            let coverage = crate::hosted_language::coverage(placement)
                .map_err(|error| format!("recognition declaration preparation: {error:?}"))?;
            *slot = Some(PreparedWhisperLanguage { request, coverage });
        }
        Ok(Self(prepared))
    }

    pub(super) fn get(
        &self,
        node: conduit_kernel::NodeId,
    ) -> Result<&PreparedWhisperLanguage, String> {
        self.0
            .get(usize::from(node.0))
            .and_then(Option::as_ref)
            .ok_or_else(|| "recognition request has no exact prepared Language".into())
    }
}

pub(super) fn execute(
    adapter: Option<&mut crate::hosted_speech_recognition::WhisperSpeechAdapter>,
    language: &PreparedWhisperLanguage,
    contract: &str,
    input: &[u8],
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<u8>, crate::hosted_speech_recognition::WhisperFailure> {
    let adapter =
        adapter.ok_or(crate::hosted_speech_recognition::WhisperFailure::MissingProvider)?;
    adapter.validate_language_declaration(&language.coverage)?;
    if contract == conduit_std_offers::WHISPER_CLIP_SPEECH_OPERATION {
        super::whisper_speech_back::execute_clip(Some(adapter), input, &language.request, cancelled)
    } else {
        super::whisper_speech_back::execute(Some(adapter), input, &language.request, cancelled)
    }
}
