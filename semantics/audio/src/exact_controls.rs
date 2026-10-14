//! Exact preparation bridge from checked physical quantities to Audio owners.
//! Unit arithmetic stays with Core; Audio constructors admit transported parts.
use crate::{AudioExactTimeOffset, AudioFrequencyHz};
use alloc::vec::Vec;
use conduit_core::{
    ExactNumericRefusal, ExactNumericWork, ExactQuantityConversionRequestRefusal, Quantity,
    RationalQuantityConversionReceipt, RationalQuantityRefusal, Unit,
};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum ExactAudioControlRefusal {
    Source(ExactQuantityConversionRequestRefusal),
    Conversion(RationalQuantityRefusal),
    Representation(ExactNumericRefusal),
    AudioProfile,
    Admission(NativeBindingRefusal),
}

pub struct ExactAudioControl<'a, T> {
    receipt: RationalQuantityConversionReceipt<'a>,
    value: T,
    canonical: Vec<u8>,
}
impl<'a, T> ExactAudioControl<'a, T> {
    pub fn receipt(&self) -> RationalQuantityConversionReceipt<'a> {
        self.receipt
    }
    pub fn value(&self) -> &T {
        &self.value
    }
    pub fn canonical(&self) -> &[u8] {
        &self.canonical
    }
}

fn parts<'a>(
    source: Quantity,
    original: &'a str,
    unit: Unit,
    symbol: &'static str,
    work: &mut ExactNumericWork,
) -> Result<(RationalQuantityConversionReceipt<'a>, u64, u64), ExactAudioControlRefusal> {
    let receipt =
        RationalQuantityConversionReceipt::from_checked(source, unit, original, symbol, work)
            .map_err(ExactAudioControlRefusal::Source)?;
    let result = receipt
        .result()
        .map_err(ExactAudioControlRefusal::Conversion)?;
    let (n, d) = result
        .coordinate()
        .checked_unsigned_ratio()
        .map_err(ExactAudioControlRefusal::Representation)?;
    let n = u64::try_from(n).map_err(|_| ExactAudioControlRefusal::AudioProfile)?;
    let d = u64::try_from(d).map_err(|_| ExactAudioControlRefusal::AudioProfile)?;
    Ok((receipt, n, d))
}

pub fn prepare_exact_audio_frequency<'a>(
    source: Quantity,
    original: &'a str,
    work: &mut ExactNumericWork,
) -> Result<ExactAudioControl<'a, AudioFrequencyHz>, ExactAudioControlRefusal> {
    let (receipt, n, d) = parts(source, original, Unit::Hertz, "Hz", work)?;
    let value = AudioFrequencyHz::new(d, n).map_err(ExactAudioControlRefusal::Admission)?;
    let canonical = value
        .encode()
        .map_err(ExactAudioControlRefusal::Admission)?;
    Ok(ExactAudioControl {
        receipt,
        value,
        canonical,
    })
}

pub fn prepare_exact_audio_time<'a>(
    source: Quantity,
    original: &'a str,
    work: &mut ExactNumericWork,
) -> Result<ExactAudioControl<'a, AudioExactTimeOffset>, ExactAudioControlRefusal> {
    let (receipt, n, d) = parts(source, original, Unit::Second, "s", work)?;
    let value = AudioExactTimeOffset::new(d, n).map_err(ExactAudioControlRefusal::Admission)?;
    let canonical = value
        .clone()
        .encode()
        .map_err(ExactAudioControlRefusal::Admission)?;
    Ok(ExactAudioControl {
        receipt,
        value,
        canonical,
    })
}
