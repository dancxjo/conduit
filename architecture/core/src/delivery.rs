use alloc::collections::VecDeque;
use serde::{Deserialize, Serialize};

pub const DELIVERY_CONTRACT_VERSION: u16 = 1;
pub const DELIVERY_CONTRACT_ID: &str = "conduit.delivery/evolution@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub enum EvolutionSemantics {
    Occurrence,
    CurrentState,
    Observation,
    SampledSignal,
    RequestIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub enum AdmissionUnit {
    Value,
    CoherentFrame,
    SignalBatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub enum DeliveryPressurePolicy {
    PreserveOrder,
    CoalesceLatest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryContract {
    pub version: u16,
    pub evolution: EvolutionSemantics,
    pub admission_unit: AdmissionUnit,
    pub pressure_policy: DeliveryPressurePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryContractRefusal {
    UnknownVersion,
    InvalidCombination,
}

impl DeliveryContract {
    pub const fn new(
        evolution: EvolutionSemantics,
        admission_unit: AdmissionUnit,
        pressure_policy: DeliveryPressurePolicy,
    ) -> Self {
        Self {
            version: DELIVERY_CONTRACT_VERSION,
            evolution,
            admission_unit,
            pressure_policy,
        }
    }

    pub const fn validate(self) -> Result<(), DeliveryContractRefusal> {
        if self.version != DELIVERY_CONTRACT_VERSION {
            return Err(DeliveryContractRefusal::UnknownVersion);
        }
        match (self.evolution, self.admission_unit, self.pressure_policy) {
            (
                EvolutionSemantics::Occurrence,
                AdmissionUnit::Value,
                DeliveryPressurePolicy::PreserveOrder,
            )
            | (EvolutionSemantics::CurrentState, AdmissionUnit::Value, _)
            | (EvolutionSemantics::CurrentState, AdmissionUnit::CoherentFrame, _)
            | (
                EvolutionSemantics::Observation,
                AdmissionUnit::Value,
                DeliveryPressurePolicy::PreserveOrder,
            )
            | (
                EvolutionSemantics::Observation,
                AdmissionUnit::CoherentFrame,
                DeliveryPressurePolicy::PreserveOrder,
            )
            | (
                EvolutionSemantics::SampledSignal,
                AdmissionUnit::SignalBatch,
                DeliveryPressurePolicy::PreserveOrder,
            )
            | (
                EvolutionSemantics::RequestIntent,
                AdmissionUnit::Value,
                DeliveryPressurePolicy::PreserveOrder,
            ) => Ok(()),
            _ => Err(DeliveryContractRefusal::InvalidCombination),
        }
    }

    pub fn semantic_digest(self) -> Result<[u8; 32], DeliveryContractRefusal> {
        self.validate()?;
        let bytes = [
            (self.version >> 8) as u8,
            self.version as u8,
            self.evolution as u8,
            self.admission_unit as u8,
            self.pressure_policy as u8,
        ];
        Ok(crate::semantic_digest(DELIVERY_CONTRACT_ID, &bytes))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryAccounting {
    pub admitted: u64,
    pub coalesced: u64,
    pub refused_pressure: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryAdmission<T> {
    Enqueued,
    CoalescedWholeValue { superseded: T },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryRefusal {
    InvalidContract,
    InvalidCapacity,
    Pressure,
    AccountingExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedDelivery<T> {
    pub reason: DeliveryRefusal,
    pub value: T,
}

/// A finite semantic queue. `T` remains the concrete owning Info type: frames
/// and signal batches are stored and replaced only as whole values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedDeliveryQueue<T> {
    contract: DeliveryContract,
    capacity: usize,
    maximum_accounted: u64,
    values: VecDeque<T>,
    accounting: DeliveryAccounting,
}

impl<T> BoundedDeliveryQueue<T> {
    pub fn new(
        contract: DeliveryContract,
        capacity: usize,
        maximum_accounted: u64,
    ) -> Result<Self, DeliveryRefusal> {
        contract
            .validate()
            .map_err(|_| DeliveryRefusal::InvalidContract)?;
        if capacity == 0 || maximum_accounted == 0 {
            return Err(DeliveryRefusal::InvalidCapacity);
        }
        Ok(Self {
            contract,
            capacity,
            maximum_accounted,
            values: VecDeque::with_capacity(capacity),
            accounting: DeliveryAccounting::default(),
        })
    }

    pub fn admit(&mut self, value: T) -> Result<DeliveryAdmission<T>, RejectedDelivery<T>> {
        if self.accounting.admitted >= self.maximum_accounted {
            return Err(RejectedDelivery {
                reason: DeliveryRefusal::AccountingExhausted,
                value,
            });
        }
        if self.values.len() < self.capacity {
            self.values.push_back(value);
            self.accounting.admitted += 1;
            return Ok(DeliveryAdmission::Enqueued);
        }
        match self.contract.pressure_policy {
            DeliveryPressurePolicy::PreserveOrder => {
                let Some(refused_pressure) = self
                    .accounting
                    .refused_pressure
                    .checked_add(1)
                    .filter(|count| *count <= self.maximum_accounted)
                else {
                    return Err(RejectedDelivery {
                        reason: DeliveryRefusal::AccountingExhausted,
                        value,
                    });
                };
                self.accounting.refused_pressure = refused_pressure;
                Err(RejectedDelivery {
                    reason: DeliveryRefusal::Pressure,
                    value,
                })
            }
            DeliveryPressurePolicy::CoalesceLatest => {
                let Some(coalesced) = self
                    .accounting
                    .coalesced
                    .checked_add(1)
                    .filter(|count| *count <= self.maximum_accounted)
                else {
                    return Err(RejectedDelivery {
                        reason: DeliveryRefusal::AccountingExhausted,
                        value,
                    });
                };
                let Some(newest) = self.values.back_mut() else {
                    return Err(RejectedDelivery {
                        reason: DeliveryRefusal::InvalidCapacity,
                        value,
                    });
                };
                let superseded = core::mem::replace(newest, value);
                self.accounting.admitted += 1;
                self.accounting.coalesced = coalesced;
                Ok(DeliveryAdmission::CoalescedWholeValue { superseded })
            }
        }
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.values.pop_front()
    }

    pub fn accounting(&self) -> DeliveryAccounting {
        self.accounting
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Renderer throttling is deliberately not semantic delivery accounting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationCoalescing {
    pub rendered: u64,
    pub presentation_updates_coalesced: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{rc::Rc, vec::Vec};
    use core::cell::RefCell;

    const ORDERED: DeliveryContract = DeliveryContract::new(
        EvolutionSemantics::Occurrence,
        AdmissionUnit::Value,
        DeliveryPressurePolicy::PreserveOrder,
    );
    const STATE: DeliveryContract = DeliveryContract::new(
        EvolutionSemantics::CurrentState,
        AdmissionUnit::Value,
        DeliveryPressurePolicy::CoalesceLatest,
    );
    const FRAME: DeliveryContract = DeliveryContract::new(
        EvolutionSemantics::CurrentState,
        AdmissionUnit::CoherentFrame,
        DeliveryPressurePolicy::CoalesceLatest,
    );

    #[derive(Debug)]
    struct OwnedValue {
        identity: u8,
        released: Rc<RefCell<Vec<u8>>>,
    }

    impl Drop for OwnedValue {
        fn drop(&mut self) {
            self.released.borrow_mut().push(self.identity);
        }
    }

    #[test]
    fn ordered_occurrences_refuse_pressure_without_overwriting() {
        let mut queue = BoundedDeliveryQueue::new(ORDERED, 1, 3).unwrap();
        assert_eq!(queue.admit(1), Ok(DeliveryAdmission::Enqueued));
        assert_eq!(
            queue.admit(2),
            Err(RejectedDelivery {
                reason: DeliveryRefusal::Pressure,
                value: 2,
            })
        );
        assert_eq!(queue.pop_front(), Some(1));
        assert_eq!(queue.accounting().refused_pressure, 1);
    }

    #[test]
    fn declared_state_coalescing_retains_the_newest_value_and_accounting() {
        let mut queue = BoundedDeliveryQueue::new(STATE, 1, 3).unwrap();
        queue.admit(1).unwrap();
        assert_eq!(
            queue.admit(2),
            Ok(DeliveryAdmission::CoalescedWholeValue { superseded: 1 })
        );
        assert_eq!(queue.pop_front(), Some(2));
        assert_eq!(queue.accounting().coalesced, 1);
    }

    #[test]
    fn accounting_exhaustion_returns_ownership_without_mutating_pending_state() {
        let mut queue = BoundedDeliveryQueue::new(STATE, 1, 2).unwrap();
        queue.admit(1).unwrap();
        queue.admit(2).unwrap();
        let accounting = queue.accounting();

        assert_eq!(
            queue.admit(3),
            Err(RejectedDelivery {
                reason: DeliveryRefusal::AccountingExhausted,
                value: 3,
            })
        );
        assert_eq!(queue.accounting(), accounting);
        assert_eq!(queue.pop_front(), Some(2));
    }

    #[test]
    fn supersession_and_cancellation_leave_exact_release_with_the_owner() {
        let released = Rc::new(RefCell::new(Vec::new()));
        let owned = |identity| OwnedValue {
            identity,
            released: Rc::clone(&released),
        };
        let mut queue = BoundedDeliveryQueue::new(STATE, 1, 3).unwrap();
        queue.admit(owned(1)).unwrap();

        let DeliveryAdmission::CoalescedWholeValue { superseded } = queue.admit(owned(2)).unwrap()
        else {
            panic!("a full coalescing queue must return its superseded owner");
        };
        assert!(released.borrow().is_empty());
        drop(superseded);
        assert_eq!(&*released.borrow(), &[1]);

        drop(queue.pop_front().expect("newest value remains pending"));
        assert_eq!(&*released.borrow(), &[1, 2]);
        assert!(queue.is_empty());
    }

    #[test]
    fn coherent_frames_are_replaced_only_as_whole_values() {
        let mut queue = BoundedDeliveryQueue::new(FRAME, 1, 3).unwrap();
        queue.admit([1, 1]).unwrap();
        queue.admit([2, 2]).unwrap();
        assert_eq!(queue.pop_front(), Some([2, 2]));
    }

    #[test]
    fn request_and_signal_contracts_reject_coalescing() {
        for contract in [
            DeliveryContract::new(
                EvolutionSemantics::RequestIntent,
                AdmissionUnit::Value,
                DeliveryPressurePolicy::CoalesceLatest,
            ),
            DeliveryContract::new(
                EvolutionSemantics::SampledSignal,
                AdmissionUnit::SignalBatch,
                DeliveryPressurePolicy::CoalesceLatest,
            ),
        ] {
            assert_eq!(
                contract.validate(),
                Err(DeliveryContractRefusal::InvalidCombination)
            );
        }
    }

    #[test]
    fn renderer_coalescing_cannot_mutate_semantic_delivery_accounting() {
        let mut queue = BoundedDeliveryQueue::new(ORDERED, 1, 3).unwrap();
        queue.admit(1).unwrap();
        let semantic = queue.accounting();
        let presentation = PresentationCoalescing {
            rendered: 1,
            presentation_updates_coalesced: 153,
        };
        assert_eq!(presentation.presentation_updates_coalesced, 153);
        assert_eq!(queue.accounting(), semantic);
        assert_eq!(queue.accounting().coalesced, 0);
    }

    #[test]
    fn delivery_meaning_changes_exact_contract_identity() {
        assert_ne!(
            ORDERED.semantic_digest().unwrap(),
            STATE.semantic_digest().unwrap()
        );
        let mut future = ORDERED;
        future.version += 1;
        assert_eq!(
            future.semantic_digest(),
            Err(DeliveryContractRefusal::UnknownVersion)
        );
    }
}
