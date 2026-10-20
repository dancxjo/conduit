//! Evidence preparation for actual frozen formant PCM and original cycle controls.
//! No feature arithmetic, neural state, silence pitch policy or renderer scheduler.
use crate::{
    carried_formant::FrozenCarriedFormant, complete_carrier::PreparedCompleteSpeechCarrier,
    semantic::*,
};
use alloc::vec::Vec;
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};
const PROGRAM: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/fargan_carrier_period_program.hex"
));
pub const SOURCE_EPOCH_FRAMES: usize = 80;
pub const ANALYSIS_RATE_HZ: u64 = 16000;
/// Required consumer ledger; this preparation does not claim those graphs ran.
pub const PERIOD_CONSUMERS: [&str; 3] = [
    "speech/fargan-feature-log-pitch",
    "speech/fargan-feature-normalized-correlation",
    "speech/fargan-conditioning-core",
];
#[derive(Debug)]
pub enum CarrierEpochRefusal {
    Native(NativeBindingRefusal),
    Program,
    Arithmetic,
    UnsupportedCycle,
    CycleBounds,
    ForeignCarrier,
    ForeignBasis,
    Geometry,
    Capacity,
    Render,
    UnrealizedAcoustics {
        frequency: bool,
        energy: bool,
        rate: bool,
        observations: bool,
    },
}
/// Nonconstructible receipt; Source arithmetic and generated Native laws both run.
pub struct PreparedFarganPeriod {
    projection: SpeechFarganPeriodProjection,
    input_bytes: Vec<u8>,
    output_bytes: Vec<u8>,
    period: u16,
}
impl PreparedFarganPeriod {
    pub fn projection(&self) -> &SpeechFarganPeriodProjection {
        &self.projection
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input_bytes
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output_bytes
    }
    pub fn source_program_hex(&self) -> &'static str {
        PROGRAM
    }
    /// Exactly one ledger value; caller must not derive separate competing values.
    pub fn period(&self) -> u16 {
        self.period
    }
    pub fn consumer_ledger(&self) -> [(&'static str, u16); 3] {
        PERIOD_CONSUMERS.map(|entry| (entry, self.period))
    }
}
pub fn prepare_fargan_period(
    cycle: &SpeechFundamentalCycle,
) -> Result<PreparedFarganPeriod, CarrierEpochRefusal> {
    use CarrierEpochRefusal::*;
    let input = SpeechFarganPeriodInput::new(cycle.clone()).map_err(|_| CycleBounds)?;
    let input_bytes = input.clone().encode().map_err(Native)?;
    let program = PortableExpressionProgram::from_canonical_hex(PROGRAM).map_err(|_| Program)?;
    let output_bytes = program.evaluate(&input_bytes).map_err(|_| Arithmetic)?;
    let raw = SpeechFarganPeriodRawProjection::decode(&output_bytes).map_err(Native)?;
    if !*raw.supported() {
        return Err(UnsupportedCycle);
    }
    // Constructor argument ordering follows canonical field labels, as in other
    // generated Speech bindings. It rechecks all exact input/projection laws.
    let projection = SpeechFarganPeriodProjection::new(
        input,
        *raw.remainder_numerator(),
        *raw.rounded_frames(),
        *raw.rounded_up(),
        *raw.whole_frames(),
    )
    .map_err(Native)?;
    let period = u16::try_from(*projection.rounded_frames()).map_err(|_| UnsupportedCycle)?;
    Ok(PreparedFarganPeriod {
        projection,
        input_bytes,
        output_bytes,
        period,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpochBlocker {
    PartialTail {
        frames: usize,
    },
    MissingCycle {
        event: usize,
    },
    ConflictingCycles {
        first_event: usize,
        other_event: usize,
    },
    UnsupportedCycle {
        event: usize,
    },
}
/// Every actual source sample remains represented, including unsupported tails.
pub struct CarrierEpoch {
    start_frame: u64,
    pcm: Vec<i16>,
    events: Vec<usize>,
    period: Option<PreparedFarganPeriod>,
    blockers: Vec<EpochBlocker>,
}
impl CarrierEpoch {
    pub fn start_frame(&self) -> u64 {
        self.start_frame
    }
    pub fn pcm(&self) -> &[i16] {
        &self.pcm
    }
    pub fn events(&self) -> &[usize] {
        &self.events
    }
    pub fn period(&self) -> Option<&PreparedFarganPeriod> {
        self.period.as_ref()
    }
    pub fn blockers(&self) -> &[EpochBlocker] {
        &self.blockers
    }
}
/// Full immutable carrier is retained by the actual frozen projection reference.
/// This is an evidence packet, never another linguistic IR or audible-output grant.
pub struct PreparedCarrierEpochs<'a> {
    frozen: &'a FrozenCarriedFormant,
    epochs: Vec<CarrierEpoch>,
}
impl<'a> PreparedCarrierEpochs<'a> {
    pub fn source(&self) -> &'a FrozenCarriedFormant {
        self.frozen
    }
    pub fn epochs(&self) -> &[CarrierEpoch] {
        &self.epochs
    }
}
pub fn prepare_carrier_epochs<'a>(
    original: &PreparedCompleteSpeechCarrier<'_, '_, '_>,
    frozen: &'a FrozenCarriedFormant,
    maximum_epochs: usize,
) -> Result<PreparedCarrierEpochs<'a>, CarrierEpochRefusal> {
    use CarrierEpochRefusal::*;
    if original.canonical_bytes() != frozen.original_bytes()
        || original.identity() != *frozen.source_identity()
    {
        return Err(ForeignCarrier);
    }
    let acoustics = original.acoustics();
    let (frequency, energy, rate, observations) = (
        acoustics.frequency.is_some(),
        acoustics.energy.is_some(),
        acoustics.rate.is_some(),
        !acoustics.frequency_observations.is_empty() || !acoustics.energy_observations.is_empty(),
    );
    if frequency || energy || rate || observations {
        return Err(UnrealizedAcoustics {
            frequency,
            energy,
            rate,
            observations,
        });
    }
    if *frozen.sample_basis().sample_rate_hz() != 8000 {
        return Err(ForeignBasis);
    }
    let spans = frozen.timing_receipts();
    let controls = frozen.control_receipts();
    if spans.len() != controls.len() {
        return Err(Geometry);
    }
    let mut end = 0u64;
    for span in spans {
        if *span.start_frame() != end {
            return Err(Geometry);
        }
        end = end.checked_add(*span.frame_count()).ok_or(Geometry)?;
    }
    if end != frozen.total_frames() {
        return Err(Geometry);
    }
    // Traversal only: frame geometry comes from the admitted Audio grid/tape.
    let count = usize::try_from(end)
        .map_err(|_| Capacity)?
        .div_ceil(SOURCE_EPOCH_FRAMES);
    if count > maximum_epochs {
        return Err(Capacity);
    }
    let mut epochs = Vec::with_capacity(count);
    let mut cursor = frozen.render.initial();
    while !cursor.is_complete() {
        let start = cursor.rendered_frames();
        let mut pcm = Vec::with_capacity(SOURCE_EPOCH_FRAMES);
        while pcm.len() < SOURCE_EPOCH_FRAMES && !cursor.is_complete() {
            let mut samples = [0i16; 8];
            let limit = samples.len().min(SOURCE_EPOCH_FRAMES - pcm.len());
            let n = frozen
                .render
                .render(&mut cursor, &mut samples[..limit])
                .map_err(|_| Render)?;
            if n == 0 {
                return Err(Render);
            }
            pcm.extend_from_slice(&samples[..n]);
        }
        let finish = cursor.rendered_frames();
        let mut blockers = Vec::new();
        if pcm.len() != SOURCE_EPOCH_FRAMES {
            blockers.push(EpochBlocker::PartialTail { frames: pcm.len() });
        }
        let mut events = Vec::new();
        let mut selected: Option<(usize, &SpeechFundamentalCycle)> = None;
        for (i, (span, (cycle, _))) in spans.iter().zip(controls).enumerate() {
            let span_end = span
                .start_frame()
                .checked_add(*span.frame_count())
                .ok_or(Geometry)?;
            if *span.start_frame() >= finish || span_end <= start {
                continue;
            }
            events.push(i);
            let Some(cycle) = cycle else {
                blockers.push(EpochBlocker::MissingCycle { event: i });
                continue;
            };
            if *cycle.request().sample_rate_hz() != 8000 {
                return Err(ForeignBasis);
            }
            let original_cycle = cycle.request().cycle();
            match selected {
                None => selected = Some((i, original_cycle)),
                Some((first, previous)) if previous != original_cycle => {
                    blockers.push(EpochBlocker::ConflictingCycles {
                        first_event: first,
                        other_event: i,
                    })
                }
                _ => {}
            }
        }
        let period = if blockers.is_empty() {
            let (event, cycle) = selected.ok_or(Geometry)?;
            match prepare_fargan_period(cycle) {
                Ok(receipt) => Some(receipt),
                Err(UnsupportedCycle | CycleBounds) => {
                    blockers.push(EpochBlocker::UnsupportedCycle { event });
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        epochs.push(CarrierEpoch {
            start_frame: start,
            pcm,
            events,
            period,
            blockers,
        });
    }
    Ok(PreparedCarrierEpochs { frozen, epochs })
}
