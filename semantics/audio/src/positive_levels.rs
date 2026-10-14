//! Positive finite control boundary over the existing exact logarithmic profile.
use crate::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeRustBinding;
/// Original positive/finite Source admission alongside its exact conversion.
pub struct AudioPositiveFiniteReceipt<I, R> {
    original: I,
    original_canonical: Vec<u8>,
    conversion: R,
}
impl<I, R> AudioPositiveFiniteReceipt<I, R> {
    pub fn original(&self) -> &I {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn conversion(&self) -> &R {
        &self.conversion
    }
}
pub type AudioPositiveRatioDecibelReceipt =
    AudioPositiveFiniteReceipt<AudioPositiveReferencedLevelRatio, AudioRatioDecibelReceipt>;
pub type AudioFiniteDecibelRatioReceipt =
    AudioPositiveFiniteReceipt<AudioFiniteDecibelLevel, AudioDecibelRatioReceipt>;

/// Admits positive Source domains before invoking the exact supported profile.
/// References, provenance and reproducible execution remain in the receipt.
pub struct PreparedPositiveFiniteDecibels {
    exact: PreparedExactPowerOfTenDecibels,
}
impl PreparedPositiveFiniteDecibels {
    pub fn new() -> Result<Self, AudioDecibelRefusal> {
        Ok(Self {
            exact: PreparedExactPowerOfTenDecibels::new()?,
        })
    }
    pub fn ratio_to_decibels(
        &self,
        canonical: &[u8],
    ) -> Result<AudioPositiveRatioDecibelReceipt, AudioDecibelRefusal> {
        let positive = AudioPositiveReferencedLevelRatio::decode(canonical)?;
        let ratio = match positive.ratio() {
            AudioPositiveLevelRatio::Amplitude(a) => {
                AudioLevelRatio::amplitude(*a.denominator(), *a.numerator())?
            }
            AudioPositiveLevelRatio::Power(p) => {
                AudioLevelRatio::power(*p.denominator(), *p.numerator())?
            }
        };
        let original = AudioReferencedLevelRatio::new(positive.basis().clone(), ratio)?;
        let receipt = self.exact.ratio_to_decibels(&original.encode()?)?;
        // Source admission establishes finite output; no infinity substitution.
        let AudioDecibelValue::Finite(value) = receipt.result().value() else {
            return Err(AudioDecibelRefusal::Source(
                AudioSourceExecutionRefusal::InvalidOutput,
            ));
        };
        AudioFiniteDecibelLevel::new(
            receipt.result().basis().clone(),
            AudioDecibelFraction::new(*value.denominator(), *value.numerator_db())?,
        )?;
        Ok(AudioPositiveFiniteReceipt {
            original: positive,
            original_canonical: canonical.to_vec(),
            conversion: receipt,
        })
    }
    pub fn decibels_to_ratio(
        &self,
        canonical: &[u8],
    ) -> Result<AudioFiniteDecibelRatioReceipt, AudioDecibelRefusal> {
        let finite = AudioFiniteDecibelLevel::decode(canonical)?;
        let original = AudioDecibelLevel::new(
            finite.basis().clone(),
            AudioDecibelValue::finite(
                *finite.value().denominator(),
                *finite.value().numerator_db(),
            )?,
        )?;
        let receipt = self.exact.decibels_to_ratio(&original.encode()?)?;
        let ratio = match receipt.result().ratio() {
            AudioLevelRatio::Amplitude(a) => {
                AudioPositiveLevelRatio::amplitude(*a.denominator(), *a.numerator())?
            }
            AudioLevelRatio::Power(p) => {
                AudioPositiveLevelRatio::power(*p.denominator(), *p.numerator())?
            }
        };
        AudioPositiveReferencedLevelRatio::new(receipt.result().basis().clone(), ratio)?;
        Ok(AudioPositiveFiniteReceipt {
            original: finite,
            original_canonical: canonical.to_vec(),
            conversion: receipt,
        })
    }
}
