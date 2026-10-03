//! One ordered utterance, fixed PCM storage, demand-driven real synthesis.
use super::*;
use crate::hosted_process::stream::{ProcessStream, StreamFailure};
use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};

#[derive(Clone, Copy)]
pub(crate) struct StreamLimits {
    pub maximum_bytes: u32,
    pub maximum_millis: u32,
    pub maximum_segments: u32,
}
impl StreamLimits {
    pub fn from_placement(placement: &PlannedGear) -> Result<Self, EspeakFailure> {
        if placement.configuration.len() != 3 {
            return Err(EspeakFailure::InvalidLimits);
        }
        let value = |key: &str, maximum: u32| {
            let values: Vec<_> = placement
                .configuration
                .iter()
                .filter(|v| v.key == key)
                .collect();
            match values.as_slice() {
                [entry] => match entry.value {
                    ConfigurationValue::U64(value) if value > 0 && value <= u64::from(maximum) => {
                        Ok(value as u32)
                    }
                    _ => Err(EspeakFailure::InvalidLimits),
                },
                _ => Err(EspeakFailure::InvalidLimits),
            }
        };
        Ok(Self {
            maximum_bytes: value(
                "maximum-output-bytes",
                conduit_tongues::MAXIMUM_STREAM_PCM_BYTES,
            )?,
            maximum_millis: value(
                "maximum-audio-millis",
                conduit_tongues::MAXIMUM_STREAM_AUDIO_MILLIS,
            )?,
            maximum_segments: value(
                "maximum-segments",
                conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS as u32,
            )?,
        })
    }
}
pub(crate) struct StreamingSpeech<'a> {
    adapter: &'a EspeakSpeechAdapter,
    limits: StreamLimits,
    process: Option<ProcessStream>,
    identity: Option<String>,
    next_sequence: u32,
    frames: u64,
    clock: u64,
    segment_bytes: usize,
    header: [u8; 44],
    block: Vec<u8>,
    text_hash: Sha256,
    pcm_hash: Sha256,
}
impl<'a> StreamingSpeech<'a> {
    pub fn prepare(
        adapter: &'a EspeakSpeechAdapter,
        placement: &PlannedGear,
    ) -> Result<Self, EspeakFailure> {
        adapter.validate_placement(placement)?;
        Ok(Self {
            adapter,
            limits: StreamLimits::from_placement(placement)?,
            process: None,
            identity: None,
            next_sequence: 0,
            frames: 0,
            clock: 0,
            segment_bytes: 0,
            header: [0; 44],
            block: Vec::with_capacity(conduit_std_offers::SPEECH_PCM_BLOCK_BYTES as usize),
            text_hash: Sha256::new(),
            pcm_hash: Sha256::new(),
        })
    }
    pub fn next(
        &mut self,
        input: &[u8],
        cancelled: impl Fn() -> bool,
    ) -> Result<Option<&[u8]>, EspeakFailure> {
        if cancelled() {
            self.process.take();
            return Err(EspeakFailure::Cancelled);
        }
        if self.process.is_none() {
            let segment = conduit_tongues::decode_speakable_segment(input)
                .map_err(|_| EspeakFailure::InvalidText)?;
            if segment.sequence != self.next_sequence
                || self.next_sequence >= self.limits.maximum_segments
                || self
                    .identity
                    .as_ref()
                    .is_some_and(|id| id != &segment.stream_identity)
                || segment.text.contains('\0')
            {
                return Err(EspeakFailure::InvalidText);
            }
            self.adapter.discovery.verify()?;
            if cancelled() {
                return Err(EspeakFailure::Cancelled);
            }
            let remaining = self.limits.maximum_bytes as usize - self.frames as usize * 2;
            let mut process = ProcessStream::start(&ProcessRequest {
                program: &self.adapter.discovery.executable,
                arguments: &self.adapter.arguments,
                environment: &self.adapter.environment,
                stdin: segment.text.as_bytes(),
                maximum_stdout_bytes: remaining + 44,
                maximum_stderr_bytes: 4096,
                timeout: self.adapter.timeout,
                require_process_group: true,
            })
            .map_err(|_| EspeakFailure::SpawnFailed)?;
            let mut read = 0;
            while read < self.header.len() {
                let count = process
                    .read(&mut self.header[read..], &cancelled)
                    .map_err(terminal)?;
                if count == 0 {
                    return Err(EspeakFailure::InvalidWav);
                }
                read += count;
            }
            // Validate the fixed PCM format independently of streaming extent.
            let mut validation = [0; 46];
            validation[..44].copy_from_slice(&self.header);
            validation[4..8].copy_from_slice(&38_u32.to_le_bytes());
            validation[40..44].copy_from_slice(&2_u32.to_le_bytes());
            wav::pcm(&validation, 2)?;
            if self.identity.is_none() {
                let digest = Sha256::digest(segment.stream_identity.as_bytes());
                self.clock = u64::from_le_bytes(digest[..8].try_into().expect("clock digest"));
            }
            self.identity.get_or_insert(segment.stream_identity);
            self.next_sequence += 1;
            self.text_hash
                .update((segment.text.len() as u32).to_le_bytes());
            self.text_hash.update(segment.text.as_bytes());
            self.segment_bytes = 0;
            self.process = Some(process);
        } else if input != [0] {
            return Err(EspeakFailure::InvalidText);
        }
        let mut payload = [0; conduit_std_offers::SPEECH_FRAMES_PER_BLOCK as usize * 2];
        let mut count = 0;
        let process = self.process.as_mut().expect("started process");
        let mut eof = false;
        while count < payload.len() {
            let size = process
                .read(&mut payload[count..], &cancelled)
                .map_err(terminal)?;
            if size == 0 {
                eof = true;
                break;
            }
            count += size;
        }
        if !count.is_multiple_of(2) {
            return Err(EspeakFailure::InvalidWav);
        }
        self.segment_bytes += count;
        if eof {
            let riff = u32::from_le_bytes(self.header[4..8].try_into().unwrap());
            let data = u32::from_le_bytes(self.header[40..44].try_into().unwrap());
            if self.segment_bytes == 0
                || !((riff == 0x7ffff024 && data == 0x7ffff000)
                    || (data as usize == self.segment_bytes
                        && riff as usize == self.segment_bytes + 36))
            {
                return Err(EspeakFailure::InvalidWav);
            }
            // Keep the completed handle until the caller pulls the segment terminal.
            if count == 0 {
                self.adapter.discovery.verify()?;
                if cancelled() {
                    self.process.take();
                    return Err(EspeakFailure::Cancelled);
                }
                self.process.take();
                return Ok(None);
            }
        }
        let next = self.frames + (count / 2) as u64;
        if next * 2 > u64::from(self.limits.maximum_bytes)
            || next * 1000 > u64::from(self.limits.maximum_millis) * 22_050
        {
            self.process.take();
            return Err(EspeakFailure::OutputOverflow);
        }
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            22_050,
            PcmChannelLayout::Mono,
            (count / 2) as u16,
            self.clock,
            self.frames,
            false,
        )
        .map_err(|_| EspeakFailure::InvalidWav)?;
        self.frames = next;
        self.pcm_hash.update(&payload[..count]);
        self.block.clear();
        self.block.extend_from_slice(&header.encode());
        self.block.extend_from_slice(&payload[..count]);
        Ok(Some(&self.block))
    }
}
fn terminal(value: StreamFailure) -> EspeakFailure {
    match value {
        StreamFailure::StdoutBoundExceeded => EspeakFailure::OutputOverflow,
        StreamFailure::Terminal(ProcessTerminal::Cancelled) => EspeakFailure::Cancelled,
        StreamFailure::Terminal(ProcessTerminal::TimedOut) => EspeakFailure::Timeout,
        _ => EspeakFailure::ProviderLost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_pcm_budget_failure_keeps_its_work_exhaustion_identity() {
        let pcm = terminal(StreamFailure::StdoutBoundExceeded);
        assert_eq!(pcm, EspeakFailure::OutputOverflow);
        assert_eq!(
            pcm.host_failure().1.code,
            conduit_kernel::FailureCode::WorkBudgetExhausted
        );
        let diagnostic = terminal(StreamFailure::Terminal(ProcessTerminal::ProviderLost(
            "stderr bound exceeded".into(),
        )));
        assert_eq!(diagnostic, EspeakFailure::ProviderLost);
        assert_eq!(
            diagnostic.host_failure().1.code,
            conduit_kernel::FailureCode::HostCallFailed
        );
    }

    #[test]
    fn stream_contract_admits_explicit_total_work_without_changing_single_shot() {
        let single = conduit_tongues::synthesize_semantic_contract();
        let stream = conduit_tongues::streaming_synthesize_semantic_contract();
        assert_eq!(
            single.kind_contract_revision.as_str(),
            "conduit.speech/synthesize@1"
        );
        assert_eq!(single.configuration.len(), 1);
        assert_eq!(
            stream.kind_contract_revision.as_str(),
            "conduit.speech/synthesize-stream@2"
        );
        assert_eq!(stream.configuration.len(), 3);
        assert_eq!(
            stream.configuration[0].default_value,
            ConfigurationValue::U64(131_072)
        );
        assert_eq!(
            single.configuration[0].rule,
            conduit_core::KindConfigurationRule::U64Range {
                minimum: 1,
                maximum: 131_072
            }
        );
        assert_eq!(
            stream.configuration[0].rule,
            conduit_core::KindConfigurationRule::U64Range {
                minimum: 1,
                maximum: 1_323_000
            }
        );
    }
}
