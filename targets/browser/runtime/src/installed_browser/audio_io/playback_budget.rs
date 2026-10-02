//! Exact, finite mixed-rate work accounting for the rational128 playback profile.
use conduit_audio::PcmFrameHeader;
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{Failure, FailureCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PlaybackBudget {
    remaining_blocks: u32,
    numerator: u128,
    denominator: u128,
}
impl PlaybackBudget {
    pub(super) fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let offer = super::playback_offer();
        if placement.configuration.len() != 2
            || placement.semantic_contract != offer.semantic_contract
            || placement.limits != offer.limits
        {
            return Err("browser playback work contract differs from its exact offer".into());
        }
        let value = |key: &str| {
            let mut entries = placement
                .configuration
                .iter()
                .filter(|entry| entry.key == key);
            match (entries.next(), entries.next()) {
                (Some(entry), None) => match entry.value {
                    ConfigurationValue::U64(value) => u32::try_from(value).ok(),
                    _ => None,
                },
                _ => None,
            }
            .ok_or_else(|| format!("missing or invalid browser playback {key}"))
        };
        Self::new(value("maximum-blocks")?, value("maximum-audio-millis")?)
    }
    pub(super) fn new(blocks: u32, millis: u32) -> Result<Self, String> {
        if blocks == 0
            || blocks > conduit_semantic_catalog::AUDIO_STREAM_MAXIMUM_BLOCKS
            || millis == 0
            || millis > conduit_semantic_catalog::AUDIO_STREAM_MAXIMUM_MILLIS
        {
            return Err("browser playback work is outside the semantic bounds".into());
        }
        let divisor = gcd(u128::from(millis), 1000);
        Ok(Self {
            remaining_blocks: blocks,
            numerator: u128::from(millis) / divisor,
            denominator: 1000 / divisor,
        })
    }
    pub(super) fn charge(&mut self, frame: &PcmFrameHeader) -> Result<(), Failure> {
        let exhausted = Failure {
            code: FailureCode::WorkBudgetExhausted,
            detail: 9,
        };
        // This distinct refusal declares the installed profile's arithmetic
        // capacity. It is not a duration estimate or a forbidden sample rate.
        let capacity = Failure {
            code: FailureCode::StorageExhausted,
            detail: 10,
        };
        if self.remaining_blocks == 0 {
            return Err(exhausted);
        }
        let rate = u128::from(frame.sample_rate_hz);
        let common = gcd(self.denominator, rate);
        let multiplier = rate / common;
        let available = self.numerator.checked_mul(multiplier).ok_or(capacity)?;
        let consumed = u128::from(frame.frame_count)
            .checked_mul(self.denominator / common)
            .ok_or(capacity)?;
        let numerator = available.checked_sub(consumed).ok_or(exhausted)?;
        let denominator = self.denominator.checked_mul(multiplier).ok_or(capacity)?;
        let divisor = gcd(numerator, denominator);
        // Commit only after every bound and checked operation succeeds.
        self.numerator = numerator / divisor;
        self.denominator = denominator / divisor;
        self.remaining_blocks -= 1;
        Ok(())
    }
}
fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn advertised_playback_placement_is_accepted_and_limit_substitution_refused() {
        let host =
            crate::installed_browser::advertisement("budget-host".into(), "budget-boot".into());
        let offer = host
            .capabilities
            .into_iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str() == super::super::PLAY_IMPLEMENTATION
            })
            .unwrap();
        assert_eq!(offer.limits, super::super::playback_offer().limits);
        let mut placement = conduit_core::planned_gear_from_parts! {
            semantic_contract: offer.semantic_contract,
            placement_id: "playback-placement".into(),
            gear_id: "playback".into(),
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: vec![
                conduit_core::ConfigurationEntry { key: "maximum-blocks".into(), value: ConfigurationValue::U64(3072) },
                conduit_core::ConfigurationEntry { key: "maximum-audio-millis".into(), value: ConfigurationValue::U64(16384) },
            ],
            host_id: "budget-host".into(),
            boot_id: "budget-boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
            capability_id: offer.capability_id,
            implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id,
            base: None,
            realization_characteristics: Vec::new(),
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            terminal_transductions: Vec::new(),
            host_calls: offer.host_calls,
            resources: Vec::new(),
            authority: Vec::new(),
            pool_references: Vec::new(),
        };
        assert!(PlaybackBudget::from_placement(&placement).is_ok());
        placement.limits.max_queue_bytes += 1;
        assert!(PlaybackBudget::from_placement(&placement).is_err());
    }
    fn frame(rate: u32, count: u16) -> PcmFrameHeader {
        PcmFrameHeader::new(
            conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
            rate,
            conduit_audio::PcmChannelLayout::Mono,
            count,
            1,
            9_000_000,
            false,
        )
        .unwrap()
    }
    #[test]
    fn mixed_common_rates_have_an_exact_boundary_independent_of_clock_position() {
        let mut budget = PlaybackBudget::new(32768, 30000).unwrap();
        for rate in [8000, 22050, 44100, 48000, 96000, 192000] {
            let mut remaining = rate * 5;
            while remaining > 0 {
                let count = remaining.min(2048);
                budget.charge(&frame(rate, count as u16)).unwrap();
                remaining -= count;
            }
        }
        assert_eq!(budget.numerator, 0);
        let before = budget;
        assert_eq!(
            budget.charge(&frame(48000, 1)).unwrap_err().code,
            FailureCode::WorkBudgetExhausted
        );
        assert_eq!(budget, before);
    }
    #[test]
    fn coprime_rate_accounting_capacity_is_distinct_and_atomic() {
        let mut budget = PlaybackBudget::new(32768, 30000).unwrap();
        let mut refused = false;
        for rate in [
            8009, 8011, 8017, 8039, 8053, 8059, 8069, 8081, 8087, 8089, 8093, 8101, 8111,
        ] {
            let before = budget;
            match budget.charge(&frame(rate, 1)) {
                Ok(()) => {}
                Err(failure) => {
                    assert_eq!(failure.code, FailureCode::StorageExhausted);
                    assert_eq!(budget, before);
                    refused = true;
                    break;
                }
            }
        }
        assert!(refused);
    }
    #[test]
    fn configured_block_ceiling_is_independent_of_duration() {
        let mut budget = PlaybackBudget::new(1, 30000).unwrap();
        budget.charge(&frame(192000, 1)).unwrap();
        let before = budget;
        assert_eq!(
            budget.charge(&frame(192000, 1)).unwrap_err().code,
            FailureCode::WorkBudgetExhausted
        );
        assert_eq!(budget, before);
        assert!(PlaybackBudget::new(32769, 30000).is_err());
        assert!(PlaybackBudget::new(1, 30001).is_err());
    }
}
