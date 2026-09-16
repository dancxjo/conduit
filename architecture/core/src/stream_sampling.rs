//! Deterministic, bounded stream sampling distinct from pressure loss.

use serde::{Deserialize, Serialize};

pub const STREAM_SAMPLING_CONTRACT_ID: &str = "conduit.flow/deterministic-sample@1";
pub const STREAM_SAMPLING_CONTRACT_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamSamplingContract {
    pub version: u16,
    /// Select one source item from each group of this size.
    pub every: u32,
    /// Zero-based selected position within each group.
    pub offset: u32,
    pub maximum_item_bytes: u64,
    /// Finite accounting horizon admitted for one sampler instance.
    pub maximum_presented_items: u64,
}

impl StreamSamplingContract {
    pub const fn new(
        every: u32,
        offset: u32,
        maximum_item_bytes: u64,
        maximum_presented_items: u64,
    ) -> Self {
        Self {
            version: STREAM_SAMPLING_CONTRACT_VERSION,
            every,
            offset,
            maximum_item_bytes,
            maximum_presented_items,
        }
    }

    pub const fn validate(self) -> Result<(), StreamSamplingRefusal> {
        if self.version != STREAM_SAMPLING_CONTRACT_VERSION {
            return Err(StreamSamplingRefusal::UnknownVersion);
        }
        if self.every == 0
            || self.offset >= self.every
            || self.maximum_item_bytes == 0
            || self.maximum_presented_items == 0
        {
            return Err(StreamSamplingRefusal::InvalidBounds);
        }
        Ok(())
    }

    pub fn semantic_digest(self) -> Result<[u8; 32], StreamSamplingRefusal> {
        self.validate()?;
        let mut bytes = [0_u8; 26];
        bytes[0..2].copy_from_slice(&self.version.to_le_bytes());
        bytes[2..6].copy_from_slice(&self.every.to_le_bytes());
        bytes[6..10].copy_from_slice(&self.offset.to_le_bytes());
        bytes[10..18].copy_from_slice(&self.maximum_item_bytes.to_le_bytes());
        bytes[18..26].copy_from_slice(&self.maximum_presented_items.to_le_bytes());
        Ok(crate::semantic_digest(STREAM_SAMPLING_CONTRACT_ID, &bytes))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamSamplingAccounting {
    pub presented: u64,
    pub selected: u64,
    pub intentionally_unselected: u64,
    pub refused_oversize: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamSamplingDisposition<T> {
    Selected { source_index: u64, value: T },
    IntentionallyUnselected { source_index: u64, value: T },
}

impl<T> StreamSamplingDisposition<T> {
    pub const fn source_index(&self) -> u64 {
        match self {
            Self::Selected { source_index, .. }
            | Self::IntentionallyUnselected { source_index, .. } => *source_index,
        }
    }

    pub fn into_value(self) -> T {
        match self {
            Self::Selected { value, .. } | Self::IntentionallyUnselected { value, .. } => value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamSamplingRefusal {
    UnknownVersion,
    InvalidBounds,
    ItemTooLarge,
    AccountingExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedStreamSample<T> {
    pub value: T,
    pub source_index: u64,
    pub reason: StreamSamplingRefusal,
}

/// One finite accounting epoch of a reviewed deterministic sampling rule.
///
/// Both selected and intentionally unselected values return exact ownership to
/// the caller. Pressure is not represented here and cannot be relabeled as
/// intentional sampling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedStreamSampler {
    contract: StreamSamplingContract,
    accounting: StreamSamplingAccounting,
}

impl BoundedStreamSampler {
    pub fn new(contract: StreamSamplingContract) -> Result<Self, StreamSamplingRefusal> {
        contract.validate()?;
        Ok(Self {
            contract,
            accounting: StreamSamplingAccounting::default(),
        })
    }

    pub fn consider<T>(
        &mut self,
        value: T,
        item_bytes: u64,
    ) -> Result<StreamSamplingDisposition<T>, RejectedStreamSample<T>> {
        let source_index = self.accounting.presented;
        if source_index >= self.contract.maximum_presented_items {
            return Err(RejectedStreamSample {
                value,
                source_index,
                reason: StreamSamplingRefusal::AccountingExhausted,
            });
        }

        if item_bytes > self.contract.maximum_item_bytes {
            let Some(presented) = self.accounting.presented.checked_add(1) else {
                return Err(RejectedStreamSample {
                    value,
                    source_index,
                    reason: StreamSamplingRefusal::AccountingExhausted,
                });
            };
            let Some(refused_oversize) = self.accounting.refused_oversize.checked_add(1) else {
                return Err(RejectedStreamSample {
                    value,
                    source_index,
                    reason: StreamSamplingRefusal::AccountingExhausted,
                });
            };
            self.accounting.presented = presented;
            self.accounting.refused_oversize = refused_oversize;
            return Err(RejectedStreamSample {
                value,
                source_index,
                reason: StreamSamplingRefusal::ItemTooLarge,
            });
        }

        let selected =
            source_index % u64::from(self.contract.every) == u64::from(self.contract.offset);
        let Some(presented) = self.accounting.presented.checked_add(1) else {
            return Err(RejectedStreamSample {
                value,
                source_index,
                reason: StreamSamplingRefusal::AccountingExhausted,
            });
        };
        if selected {
            let Some(selected_count) = self.accounting.selected.checked_add(1) else {
                return Err(RejectedStreamSample {
                    value,
                    source_index,
                    reason: StreamSamplingRefusal::AccountingExhausted,
                });
            };
            self.accounting.presented = presented;
            self.accounting.selected = selected_count;
            Ok(StreamSamplingDisposition::Selected {
                source_index,
                value,
            })
        } else {
            let Some(unselected) = self.accounting.intentionally_unselected.checked_add(1) else {
                return Err(RejectedStreamSample {
                    value,
                    source_index,
                    reason: StreamSamplingRefusal::AccountingExhausted,
                });
            };
            self.accounting.presented = presented;
            self.accounting.intentionally_unselected = unselected;
            Ok(StreamSamplingDisposition::IntentionallyUnselected {
                source_index,
                value,
            })
        }
    }

    pub const fn accounting(&self) -> StreamSamplingAccounting {
        self.accounting
    }

    pub const fn contract(&self) -> StreamSamplingContract {
        self.contract
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::*;

    const EVERY_THIRD: StreamSamplingContract = StreamSamplingContract::new(3, 1, 64, 6);

    #[test]
    fn reviewed_rule_selects_exact_source_indices_and_accounts_omissions() {
        let mut sampler = BoundedStreamSampler::new(EVERY_THIRD).unwrap();
        let dispositions = (0_u8..6)
            .map(|value| sampler.consider(value, 1).unwrap())
            .collect::<alloc::vec::Vec<_>>();

        let selected = dispositions
            .iter()
            .filter_map(|disposition| match disposition {
                StreamSamplingDisposition::Selected {
                    source_index,
                    value,
                } => Some((*source_index, *value)),
                StreamSamplingDisposition::IntentionallyUnselected { .. } => None,
            })
            .collect::<alloc::vec::Vec<_>>();
        assert_eq!(selected, [(1, 1), (4, 4)]);
        assert_eq!(
            sampler.accounting(),
            StreamSamplingAccounting {
                presented: 6,
                selected: 2,
                intentionally_unselected: 4,
                refused_oversize: 0,
            }
        );
    }

    #[test]
    fn oversize_and_exhaustion_return_exact_ownership_with_distinct_reasons() {
        let mut sampler =
            BoundedStreamSampler::new(StreamSamplingContract::new(1, 0, 4, 1)).unwrap();
        assert_eq!(
            sampler.consider(String::from("owner-a"), 5),
            Err(RejectedStreamSample {
                value: String::from("owner-a"),
                source_index: 0,
                reason: StreamSamplingRefusal::ItemTooLarge,
            })
        );
        assert_eq!(
            sampler.consider(String::from("owner-b"), 4),
            Err(RejectedStreamSample {
                value: String::from("owner-b"),
                source_index: 1,
                reason: StreamSamplingRefusal::AccountingExhausted,
            })
        );
        assert_eq!(sampler.accounting().refused_oversize, 1);
    }

    #[test]
    fn invalid_rules_and_changed_meaning_have_distinct_contract_identity() {
        for contract in [
            StreamSamplingContract::new(0, 0, 1, 1),
            StreamSamplingContract::new(2, 2, 1, 1),
            StreamSamplingContract::new(1, 0, 0, 1),
            StreamSamplingContract::new(1, 0, 1, 0),
        ] {
            assert_eq!(
                contract.validate(),
                Err(StreamSamplingRefusal::InvalidBounds)
            );
        }
        let first = StreamSamplingContract::new(3, 1, 64, 6);
        let second = StreamSamplingContract::new(3, 2, 64, 6);
        assert_ne!(
            first.semantic_digest().unwrap(),
            second.semantic_digest().unwrap()
        );
    }

    #[test]
    fn sampling_vocabulary_cannot_report_pressure_or_supersession() {
        let source = include_str!("stream_sampling.rs");
        assert!(!source.contains(concat!("Delivery", "PressurePolicy")));
        assert!(!source.contains(concat!("Coalesced", "WholeValue")));
    }
}
