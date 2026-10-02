use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, AUDIO_PCM_INFO_ID};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, ArtifactId, CancellationTransduction,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, ImplementationId, Kind,
    KindIdentity, KindSemanticLaw, NormalCloseTransduction, PlannedGear, PortDescriptor,
    PortDirection, PortTemporal, Quantity, QuantityUnit, TerminalTransductionProfile,
    FREQUENCY_INFO_ID, UNIT_INFO_ID,
};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId, ValueRef, ValueStorage,
};
use conduit_plot::{KindProjection, ProfileCatalog};

const SOURCE_KIND: &str = "conduit-test/frequency-source";
const SOURCE_REVISION: &str = "conduit-test/frequency-source@1";
const SOURCE_IMPLEMENTATION: &str = "conduit-test/frequency-source-kernel@1";
const DISTANCE_SOURCE_KIND: &str = "conduit-test/distance-source";
const DISTANCE_SOURCE_REVISION: &str = "conduit-test/distance-source@1";
const DISTANCE_SOURCE_IMPLEMENTATION: &str = "conduit-test/distance-source-kernel@1";
const SINK_KIND: &str = "conduit-test/tone-pcm-sink";
const SINK_REVISION: &str = "conduit-test/tone-pcm-sink@1";
const SINK_IMPLEMENTATION: &str = "conduit-test/tone-pcm-sink-kernel@1";
const CANCEL_KIND: &str = "conduit-test/cancellation-source";
const CANCEL_REVISION: &str = "conduit-test/cancellation-source@1";
const CANCEL_IMPLEMENTATION: &str = "conduit-test/cancellation-source-kernel@1";
const RECOVERY_KIND: &str = "conduit-test/tone-terminal-recovery";
const RECOVERY_REVISION: &str = "conduit-test/tone-terminal-recovery@1";
const RECOVERY_IMPLEMENTATION: &str = "conduit-test/tone-terminal-recovery-kernel@1";
const CLOSE_KIND: &str = "conduit-test/normal-close-sink";
const CLOSE_REVISION: &str = "conduit-test/normal-close-sink@1";
const CLOSE_IMPLEMENTATION: &str = "conduit-test/normal-close-sink-kernel@1";
pub(super) static SOURCE_FACTORY: BackFactory = BackFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
};
pub(super) static DISTANCE_SOURCE_FACTORY: BackFactory = BackFactory {
    implementation_id: DISTANCE_SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_distance_source,
};
pub(super) static SINK_FACTORY: BackFactory = BackFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget: sink_budget,
    prepare: prepare_sink,
};
pub(super) static CANCEL_FACTORY: BackFactory = BackFactory {
    implementation_id: CANCEL_IMPLEMENTATION,
    budget: cancel_budget,
    prepare: prepare_cancel,
};
pub(super) static RECOVERY_FACTORY: BackFactory = BackFactory {
    implementation_id: RECOVERY_IMPLEMENTATION,
    budget: recovery_budget,
    prepare: prepare_recovery,
};
pub(super) static CLOSE_FACTORY: BackFactory = BackFactory {
    implementation_id: CLOSE_IMPLEMENTATION,
    budget: close_budget,
    prepare: prepare_close,
};
pub(super) struct FrequencySourceBack {
    values: [ValueRef; 2],
    next: usize,
}
pub(super) struct DistanceSourceBack {
    values: [ValueRef; 2],
    next: usize,
}
pub(super) struct TonePcmSinkBack {
    first: [u8; 32],
    seen: u8,
}
pub(super) struct CancellationSourceBack {
    value: ValueRef,
    emitted: bool,
}
pub(super) struct ToneTerminalRecoveryBack {
    seen: bool,
}
pub(super) struct NormalCloseSinkBack;

impl<const P: usize> StepBack<P> for FrequencySourceBack {
    fn step(&mut self, io: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        if let Some(value) = self.values.get(self.next).copied() {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send(PortId(0), value).unwrap();
            self.next += 1;
            StepOutcome::Progress
        } else {
            StepOutcome::Complete
        }
    }
}
impl<const P: usize> StepBack<P> for DistanceSourceBack {
    fn step(&mut self, io: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        if let Some(value) = self.values.get(self.next).copied() {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send(PortId(0), value).unwrap();
            self.next += 1;
            StepOutcome::Progress
        } else {
            StepOutcome::Complete
        }
    }
}
impl<const P: usize> StepBack<P> for TonePcmSinkBack {
    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(31);
            };
            let Ok((h, p)) = PcmFrameHeader::decode_frame(bytes) else {
                return fail(32);
            };
            if h.representation != PcmSampleRepresentation::Signed16LittleEndian
                || h.sample_rate_hz != 48_000
                || h.layout != PcmChannelLayout::Mono
                || h.frame_count != conduit_semantic_catalog::AUDIO_TONE_PCM_FRAMES
                || h.start_frame != u64::from(self.seen) * u64::from(h.frame_count)
                || p.len() != 32
                || self.seen >= 3
            {
                return fail(33);
            }
            if self.seen == 0 {
                self.first.copy_from_slice(p)
            } else if p == self.first {
                return fail(34);
            }
            io.consume(PortId(0)).unwrap();
            self.seen += 1;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if !(2..=3).contains(&self.seen) {
                return fail(35);
            }
            io.consume_closed(PortId(0)).unwrap();
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}
impl<const P: usize> StepBack<P> for CancellationSourceBack {
    fn step(&mut self, io: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), self.value).unwrap();
        self.emitted = true;
        StepOutcome::Progress
    }
}
impl<const P: usize> StepBack<P> for ToneTerminalRecoveryBack {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        Some(AssignedTerminalTransduction {
            input: PortId(0),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::NotAccepted,
            abnormal: AssignedAbnormalTransduction::Recover,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }

    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        if self.seen {
            return StepOutcome::Complete;
        }
        if io.input(PortId(0)).is_some() {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(41);
            };
            if self.seen
                || conduit_audio::AudioToneTerminal::decode(bytes)
                    != Ok(conduit_audio::AudioToneTerminal::Cancelled)
            {
                return fail(42);
            }
            io.consume(PortId(0)).unwrap();
            self.seen = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
}
impl<const P: usize> StepBack<P> for NormalCloseSinkBack {
    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(51);
            };
            if !bytes.is_empty() {
                return fail(52);
            }
            io.consume(PortId(0)).unwrap();
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}
const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
fn port(
    name: &str,
    kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}
fn offer(
    kind: &str,
    revision: &str,
    implementation: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> CapabilityOffer {
    conduit_core::capability_offer_from_parts! {
        semantic_contract: Default::default(),
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(implementation),
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-std-host/audio-tone-proof@1"),
        },
        inputs,
        outputs,
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_semantic_catalog::AUDIO_TONE_PCM_BLOCK_BYTES,
        },
    }
}
pub(super) fn source_offer() -> CapabilityOffer {
    offer(
        SOURCE_KIND,
        SOURCE_REVISION,
        SOURCE_IMPLEMENTATION,
        Vec::new(),
        vec![port(
            "frequency",
            FREQUENCY_INFO_ID,
            PortDirection::Output,
            PortTemporal::Current,
        )],
    )
}
pub(super) fn distance_source_offer() -> CapabilityOffer {
    offer(
        DISTANCE_SOURCE_KIND,
        DISTANCE_SOURCE_REVISION,
        DISTANCE_SOURCE_IMPLEMENTATION,
        Vec::new(),
        vec![port(
            "distance",
            conduit_core::DISTANCE_INFO_ID,
            PortDirection::Output,
            PortTemporal::Value,
        )],
    )
}
pub(super) fn sink_offer() -> CapabilityOffer {
    offer(
        SINK_KIND,
        SINK_REVISION,
        SINK_IMPLEMENTATION,
        vec![port(
            "audio",
            AUDIO_PCM_INFO_ID,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        Vec::new(),
    )
}
pub(super) fn cancel_offer() -> CapabilityOffer {
    offer(
        CANCEL_KIND,
        CANCEL_REVISION,
        CANCEL_IMPLEMENTATION,
        Vec::new(),
        vec![port(
            "request",
            conduit_core::CANCELLATION_REQUEST_INFO_ID,
            PortDirection::Output,
            PortTemporal::Value,
        )],
    )
}
pub(super) fn recovery_offer() -> CapabilityOffer {
    offer(
        RECOVERY_KIND,
        RECOVERY_REVISION,
        RECOVERY_IMPLEMENTATION,
        vec![recovery_input()],
        vec![recovery_output()],
    )
}

fn recovery_input() -> PortDescriptor {
    let terminal = conduit_audio::audio_tone_terminal_kind_id();
    PortDescriptor {
        port_id: port_id("terminal"),
        value_kind: terminal.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: Some(terminal),
    }
}

fn recovery_output() -> PortDescriptor {
    port(
        "recovered",
        UNIT_INFO_ID,
        PortDirection::Output,
        PortTemporal::Value,
    )
}

fn recovery_kind() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(RECOVERY_KIND),
        kind_contract_revision: KindIdentity::from(RECOVERY_REVISION),
        inputs: vec![recovery_input()],
        outputs: vec![recovery_output()],
        configuration: Vec::new(),
        semantic_laws: vec![KindSemanticLaw::TerminalTransduction(
            TerminalTransductionProfile {
                input_port_id: port_id("terminal"),
                output_port_id: port_id("recovered"),
                normal_close: NormalCloseTransduction::NotAccepted,
                abnormal: AbnormalTerminalTransduction::Recover,
                cancellation: CancellationTransduction::NotCancellable,
            },
        )],
        limits: recovery_offer().limits,
    }
}
pub(super) fn close_offer() -> CapabilityOffer {
    offer(
        CLOSE_KIND,
        CLOSE_REVISION,
        CLOSE_IMPLEMENTATION,
        vec![port(
            "closed",
            conduit_core::UNIT_INFO_ID,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        Vec::new(),
    )
}
pub(super) fn install_catalog(c: &mut ProfileCatalog) {
    for (k, r, i, o) in [
        (
            kind_id(SOURCE_KIND),
            KindIdentity::from(SOURCE_REVISION),
            Vec::new(),
            source_offer().outputs,
        ),
        (
            kind_id(DISTANCE_SOURCE_KIND),
            KindIdentity::from(DISTANCE_SOURCE_REVISION),
            Vec::new(),
            distance_source_offer().outputs,
        ),
        (
            kind_id(SINK_KIND),
            KindIdentity::from(SINK_REVISION),
            sink_offer().inputs,
            Vec::new(),
        ),
        (
            kind_id(CANCEL_KIND),
            KindIdentity::from(CANCEL_REVISION),
            Vec::new(),
            cancel_offer().outputs,
        ),
        (
            kind_id(CLOSE_KIND),
            KindIdentity::from(CLOSE_REVISION),
            close_offer().inputs,
            Vec::new(),
        ),
    ] {
        c.insert(KindProjection {
            kind_id: k,
            kind_contract_revision: r,
            inputs: i,
            outputs: o,
            configuration: Vec::new(),
        })
        .unwrap()
    }
    c.insert_kind(recovery_kind()).unwrap();
}
fn source_budget(_: &PlannedGear) -> Result<BackBudget, String> {
    Ok(BackBudget {
        value_items: 2,
        value_bytes: 18,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: 9,
    })
}
fn prepare_source(
    _: &PlannedGear,
    s: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let a = s
        .store(&Quantity::new(220, QuantityUnit::Hertz).encode())
        .map_err(|e| format!("{e:?}"))?;
    let b = s
        .store(&Quantity::new(880, QuantityUnit::Hertz).encode())
        .map_err(|e| format!("{e:?}"))?;
    Ok(InstalledBack::TestFrequencySource(FrequencySourceBack {
        values: [a, b],
        next: 0,
    }))
}
fn prepare_distance_source(
    _: &PlannedGear,
    s: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let a = s
        .store(&Quantity::new(0, QuantityUnit::Centimeter).encode())
        .map_err(|e| format!("{e:?}"))?;
    let b = s
        .store(&Quantity::new(30, QuantityUnit::Centimeter).encode())
        .map_err(|e| format!("{e:?}"))?;
    Ok(InstalledBack::TestDistanceSource(DistanceSourceBack {
        values: [a, b],
        next: 0,
    }))
}
fn sink_budget(_: &PlannedGear) -> Result<BackBudget, String> {
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: conduit_semantic_catalog::AUDIO_TONE_PCM_BLOCK_BYTES,
    })
}
fn prepare_sink(
    _: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    Ok(InstalledBack::TestTonePcmSink(TonePcmSinkBack {
        first: [0; 32],
        seen: 0,
    }))
}

fn cancel_budget(_: &PlannedGear) -> Result<BackBudget, String> {
    Ok(BackBudget {
        value_items: 1,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: 0,
    })
}
fn prepare_cancel(
    _: &PlannedGear,
    store: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let value = store
        .store(&[])
        .map_err(|error| format!("store cancellation request: {error:?}"))?;
    Ok(InstalledBack::TestCancellationSource(
        CancellationSourceBack {
            value,
            emitted: false,
        },
    ))
}
fn recovery_budget(_: &PlannedGear) -> Result<BackBudget, String> {
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: conduit_audio::AUDIO_TONE_TERMINAL_ENCODED_LEN as u32,
    })
}
fn prepare_recovery(
    _: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    Ok(InstalledBack::TestToneTerminalRecovery(
        ToneTerminalRecoveryBack { seen: false },
    ))
}
fn close_budget(_: &PlannedGear) -> Result<BackBudget, String> {
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: 0,
    })
}
fn prepare_close(
    _: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    Ok(InstalledBack::TestNormalCloseSink(NormalCloseSinkBack))
}
