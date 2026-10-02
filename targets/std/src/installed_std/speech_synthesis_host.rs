//! Per-placement synthesis ownership and bounded pull-based PCM delivery.
use super::{speech_synthesis_back, InstalledScheduler};
use crate::hosted_speech_synthesis::EspeakSpeechAdapter;
use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_core::{PlanFragment, PlannedGear};
use conduit_kernel::{
    scheduler::HostCallRequest, BoundedValueRef, Failure, FailureCode, HostCallDisposition,
    HostCallOutcome,
};

pub(super) enum SpeechHost<'a> {
    Proof(speech_synthesis_back::FakeSpeechHost),
    Espeak {
        adapter: &'a EspeakSpeechAdapter,
        placement: &'a PlannedGear,
        pcm: Vec<u8>,
        block: Vec<u8>,
        maximum: usize,
        next: Option<usize>,
    },
}

pub(super) fn prepare_hosts<'a>(
    fragment: &'a PlanFragment,
    adapter: Option<&'a EspeakSpeechAdapter>,
) -> Result<Vec<Option<SpeechHost<'a>>>, String> {
    let proof = speech_synthesis_back::prepare_fake_hosts(fragment)?;
    fragment
        .placements
        .iter()
        .zip(proof)
        .map(|(placement, proof)| {
            if let Some(proof) = proof {
                return Ok(Some(SpeechHost::Proof(proof)));
            }
            if placement.implementation_id.as_str()
                != conduit_std_offers::ESPEAK_SPEECH_IMPLEMENTATION
            {
                return Ok(None);
            }
            speech_synthesis_back::validate(placement)?;
            let adapter = adapter.ok_or("planned eSpeak provider is not initialized")?;
            adapter
                .validate_placement(placement)
                .map_err(|error| format!("speech provider admission: {error:?}"))?;
            Ok(Some(SpeechHost::Espeak {
                adapter,
                placement,
                pcm: Vec::with_capacity(conduit_tongues::MAXIMUM_PCM_BYTES as usize),
                block: Vec::with_capacity(conduit_std_offers::SPEECH_PCM_BLOCK_BYTES as usize),
                maximum: speech_synthesis_back::maximum_output_bytes(placement)? as usize,
                next: None,
            }))
        })
        .collect()
}

impl SpeechHost<'_> {
    fn next(
        &mut self,
        input: &[u8],
        control: &crate::RunControl,
    ) -> Result<Option<&[u8]>, (HostCallDisposition, Failure)> {
        match self {
            Self::Proof(host) => host
                .execute(input)
                .map_err(|_| failed(FailureCode::InvalidInput, 1)),
            Self::Espeak {
                adapter,
                placement,
                pcm,
                block,
                maximum,
                next,
            } => {
                if next.is_none() {
                    adapter
                        .synthesize_into(placement, input, pcm, || control.stop_requested())
                        .map_err(|error| error.host_failure())?;
                    if pcm.is_empty() || pcm.len() > *maximum || pcm.len() % 2 != 0 {
                        return Err(failed(FailureCode::WorkBudgetExhausted, 2));
                    }
                    *next = Some(0);
                } else if input != [0] {
                    return Err(failed(FailureCode::InvalidInput, 3));
                }
                let start = next.expect("synthesis initialized its bounded PCM");
                if start == pcm.len() {
                    *next = None;
                    pcm.clear();
                    return Ok(None);
                }
                let end = (start + usize::from(conduit_std_offers::SPEECH_FRAMES_PER_BLOCK) * 2)
                    .min(pcm.len());
                let header = PcmFrameHeader::new(
                    PcmSampleRepresentation::Signed16LittleEndian,
                    22_050,
                    PcmChannelLayout::Mono,
                    ((end - start) / 2) as u16,
                    0x4553_5045_414b,
                    (start / 2) as u64,
                    false,
                )
                .map_err(|_| failed(FailureCode::InvalidInput, 4))?;
                block.clear();
                block.extend_from_slice(&header.encode());
                block.extend_from_slice(&pcm[start..end]);
                *next = Some(end);
                Ok(Some(block))
            }
        }
    }

    pub(super) fn complete(
        &mut self,
        scheduler: &mut InstalledScheduler,
        request: HostCallRequest,
        maximum_output_bytes: u32,
        control: &crate::RunControl,
    ) -> Result<(), String> {
        let input = scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read synthesis input: {error:?}"))?;
        let result = self.next(input, control);
        let outcome = match result {
            Ok(block) => {
                let output = block
                    .map(|block| {
                        let value = scheduler
                            .store_host_value(block)
                            .map_err(|error| format!("store speech PCM: {error:?}"))?;
                        BoundedValueRef::new(value, maximum_output_bytes)
                            .map_err(|error| format!("bound speech PCM: {error:?}"))
                    })
                    .transpose()?;
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output,
                    failure: None,
                }
            }
            Err((disposition, failure)) => HostCallOutcome {
                disposition,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(|error| format!("complete speech synthesis: {error:?}"))
    }
}

const fn failed(code: FailureCode, detail: u16) -> (HostCallDisposition, Failure) {
    (HostCallDisposition::Failed, Failure { code, detail })
}
