//! Requested retained storage owned by exact AI Native profile objects.
//! Each owned clone is counted; shared Arc owners are deduplicated by the factory.
//! Root structures, allocator bookkeeping and stack are separate charges.
use super::*;
use core::mem::size_of;
fn checked_type_heap(ty: &CheckedNativeType) -> usize {
    let mut total = ty
        .name
        .capacity()
        .saturating_add(ty.identity.owned_heap_bytes())
        .saturating_add(ty.value_type.owned_heap_bytes())
        .saturating_add(
            ty.value_contracts
                .capacity()
                .saturating_mul(size_of::<conduit_plot::NativeTypeValueContract>()),
        )
        .saturating_add(
            ty.invariants
                .capacity()
                .saturating_mul(size_of::<conduit_plot::PortableExpressionProgram>()),
        );
    for contract in &ty.value_contracts {
        total = total
            .saturating_add(contract.representation_path.capacity())
            .saturating_add(contract.contract.owned_heap_bytes());
    }
    for invariant in &ty.invariants {
        total = total.saturating_add(invariant.owned_heap_bytes());
    }
    total
}
impl PreparedNativeProfile {
    pub fn owned_heap_bytes(&self) -> usize {
        self.context.iter().fold(
            checked_type_heap(&self.checked)
                .saturating_add(
                    self.context
                        .capacity()
                        .saturating_mul(size_of::<CheckedNativeType>()),
                )
                .saturating_add(self.candidate.owned_heap_bytes())
                .saturating_add(self.candidate_kind.owned_heap_bytes())
                .saturating_add(self.native_kind.owned_heap_bytes())
                .saturating_add(self.identity.capacity()),
            |total, ty| total.saturating_add(checked_type_heap(ty)),
        )
    }
}
impl NativeProfileBack {
    /// Conservative retained requested payload inventory; preserves upstream
    /// evaluator shared-owner/header/tail-padding overcounts.
    pub fn owned_heap_bytes(&self) -> usize {
        let mut total = self
            .candidate
            .owned_heap_bytes()
            .saturating_add(self.candidate_prefix.capacity())
            .saturating_add(self.native.owned_heap_bytes())
            .saturating_add(self.structure.owned_heap_bytes())
            .saturating_add(self.native_prefix.capacity())
            .saturating_add(self.output.capacity())
            .saturating_add(self.laws.capacity().saturating_mul(size_of::<NestedLaw>()));
        for law in &self.laws {
            total = total
                .saturating_add(law.value_type.owned_heap_bytes())
                .saturating_add(law.prefix.capacity())
                .saturating_add(law.scratch.capacity())
                .saturating_add(
                    law.invariants
                        .capacity()
                        .saturating_mul(size_of::<PreparedPortableExpressionEvaluator>()),
                );
            for invariant in &law.invariants {
                total = total.saturating_add(invariant.owned_heap_bytes());
            }
        }
        total
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NativeProfilePreparationLimits {
    pub maximum_preparation_requested_bytes: usize,
    pub maximum_retained_heap_bytes: usize,
    pub maximum_decoded_program_bytes: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct NativeProfilePreparationReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
    /// Conservative post-preparation inventory; evaluator shared owners may overcount.
    pub retained_accounted_heap_bytes: usize,
}
#[derive(Debug)]
pub enum NativeProfilePreparationRefusal {
    Capacity,
    Overflow,
    Envelope,
    Structure(StructuredInfoRefusal),
    Traversal(crate::native_traversal::TraversalStorageRefusal),
    ContractStorage(conduit_core::ValueContractStorageRefusal),
    Expression(conduit_plot::PreparedExpressionStorageRefusal),
    ExpressionProgram(conduit_plot::PortableExpressionEvaluationRefusal),
}
struct Budget {
    limits: NativeProfilePreparationLimits,
    preparation: usize,
    retained: usize,
}
impl Budget {
    fn charge(
        &mut self,
        preparation: usize,
        retained: usize,
    ) -> Result<(), NativeProfilePreparationRefusal> {
        self.preparation = self
            .preparation
            .checked_add(preparation)
            .ok_or(NativeProfilePreparationRefusal::Overflow)?;
        self.retained = self
            .retained
            .checked_add(retained)
            .ok_or(NativeProfilePreparationRefusal::Overflow)?;
        if self.preparation > self.limits.maximum_preparation_requested_bytes
            || self.retained > self.limits.maximum_retained_heap_bytes
        {
            return Err(NativeProfilePreparationRefusal::Capacity);
        }
        Ok(())
    }
    fn array<T>(
        &mut self,
        count: usize,
        retained: bool,
    ) -> Result<(), NativeProfilePreparationRefusal> {
        let bytes = count
            .checked_mul(size_of::<T>())
            .ok_or(NativeProfilePreparationRefusal::Overflow)?;
        self.charge(bytes, if retained { bytes } else { 0 })
    }
    fn value(
        &mut self,
        ty: &StructuredInfoType,
        maximum: usize,
    ) -> Result<PreparedStructuredValueValidator, NativeProfilePreparationRefusal> {
        let reservation = PreparedStructuredValueValidator::storage_reservation(ty, maximum)
            .map_err(NativeProfilePreparationRefusal::Traversal)?;
        self.charge(
            reservation.preparation_requested_bytes_bound,
            reservation.retained_heap_bytes_bound,
        )?;
        PreparedStructuredValueValidator::new_with_storage_limits(
            ty,
            maximum,
            reservation.preparation_requested_bytes_bound,
            reservation.retained_heap_bytes_bound,
        )
        .map(|(value, _)| value)
        .map_err(NativeProfilePreparationRefusal::Traversal)
    }
    fn prefix(
        &mut self,
        ty: &StructuredInfoType,
    ) -> Result<Vec<u8>, NativeProfilePreparationRefusal> {
        let bytes = ty
            .canonical_byte_length()
            .map_err(NativeProfilePreparationRefusal::Structure)?;
        self.charge(bytes, bytes)?;
        ty.canonical_bytes()
            .map_err(NativeProfilePreparationRefusal::Structure)
    }
    fn output(&mut self, bytes: usize) -> Result<Vec<u8>, NativeProfilePreparationRefusal> {
        self.charge(bytes, bytes)?;
        Ok(vec![0; bytes])
    }
}
impl NativeProfileBack {
    /// Factory-only entrance after exact selected Source offer/placement verification.
    /// The incoming immutable profile's own retained storage is charged once by
    /// the aggregate owner, separately from this Back's clones and scratch.
    pub(crate) fn prepare_selected_with_storage_limits(
        profile: &PreparedNativeProfile,
        flow: bool,
        limits: NativeProfilePreparationLimits,
    ) -> Result<(Self, NativeProfilePreparationReceipt), NativeProfilePreparationRefusal> {
        let mut budget = Budget {
            limits,
            preparation: 0,
            retained: 0,
        };
        let count = profile.checked.value_contracts.len();
        budget.array::<(String, CheckedValueContract)>(count, false)?;
        for contract in &profile.checked.value_contracts {
            let reservation = contract
                .contract
                .clone_storage_reservation()
                .map_err(NativeProfilePreparationRefusal::ContractStorage)?;
            budget.charge(
                contract
                    .representation_path
                    .len()
                    .checked_add(reservation.preparation_requested_bytes_bound)
                    .ok_or(NativeProfilePreparationRefusal::Overflow)?,
                0,
            )?;
        }
        let mut contracts = Vec::with_capacity(count);
        for contract in &profile.checked.value_contracts {
            contracts.push((
                contract.representation_path.clone(),
                contract.contract.clone(),
            ));
        }
        let candidate = budget.value(&profile.candidate, profile.candidate_maximum as usize)?;
        let candidate_prefix = budget.prefix(&profile.candidate)?;
        let reservation = PreparedStructuredContractValidator::storage_reservation(
            &profile.checked.value_type,
            profile.maximum as usize,
            &contracts,
        )
        .map_err(NativeProfilePreparationRefusal::Traversal)?;
        budget.charge(
            reservation.preparation_requested_bytes_bound,
            reservation.retained_heap_bytes_bound,
        )?;
        let (native, _) = PreparedStructuredContractValidator::new_with_storage_limits(
            &profile.checked.value_type,
            profile.maximum as usize,
            &contracts,
            reservation.preparation_requested_bytes_bound,
            reservation.retained_heap_bytes_bound,
        )
        .map_err(NativeProfilePreparationRefusal::Traversal)?;
        let structure = budget.value(&profile.checked.value_type, profile.maximum as usize)?;
        let native_prefix = budget.prefix(&profile.checked.value_type)?;
        let output = budget.output(profile.maximum as usize)?;
        let law_count = profile
            .context
            .iter()
            .filter(|ty| !ty.invariants.is_empty())
            .count();
        budget.array::<NestedLaw>(law_count, true)?;
        let mut laws = Vec::with_capacity(law_count);
        for ty in &profile.context {
            if ty.invariants.is_empty() {
                continue;
            }
            // The compatibility helper owns exactly three Type clones while
            // delegating the selected Plot owner's allocation-free transport
            // extent computation. Charge those temporary owners before call.
            budget.charge(
                ty.value_type
                    .owned_heap_bytes()
                    .checked_mul(3)
                    .ok_or(NativeProfilePreparationRefusal::Overflow)?,
                0,
            )?;
            let maximum =
                crate::transport_envelope::maximum_prepared_transport_value_bytes(&ty.value_type)
                    .map_err(NativeProfilePreparationRefusal::ExpressionProgram)?
                    as usize;
            if maximum > MAXIMUM_NATIVE_PROFILE_VALUE_BYTES {
                return Err(NativeProfilePreparationRefusal::Envelope);
            }
            let prefix = budget.prefix(&ty.value_type)?;
            let scratch = budget.output(maximum)?;
            budget.charge(
                ty.value_type.owned_heap_bytes(),
                ty.value_type.owned_heap_bytes(),
            )?;
            let value_type = ty.value_type.clone();
            budget.array::<PreparedPortableExpressionEvaluator>(ty.invariants.len(), true)?;
            let mut invariants = Vec::with_capacity(ty.invariants.len());
            for program in &ty.invariants {
                let remaining_preparation =
                    limits.maximum_preparation_requested_bytes - budget.preparation;
                let remaining_retained = limits.maximum_retained_heap_bytes - budget.retained;
                let decoded = program.owned_heap_bytes();
                let evaluator_limit = remaining_preparation
                    .checked_add(decoded)
                    .ok_or(NativeProfilePreparationRefusal::Overflow)?;
                let (evaluator, receipt) =
                    PreparedPortableExpressionEvaluator::new_with_storage_limits(
                        program,
                        limits.maximum_decoded_program_bytes,
                        evaluator_limit,
                        remaining_retained,
                    )
                    .map_err(NativeProfilePreparationRefusal::Expression)?;
                budget.charge(
                    receipt.preparation_requested_bytes_bound,
                    receipt.retained_heap_bytes_bound,
                )?;
                invariants.push(evaluator);
            }
            laws.push(NestedLaw {
                value_type,
                prefix,
                scratch,
                invariants,
            });
        }
        let value = Self {
            candidate,
            candidate_prefix,
            native,
            structure,
            native_prefix,
            output,
            output_length: 0,
            laws,
            flow,
            staged: false,
            finished: false,
            cancelled: false,
            committed_frames: 0,
        };
        let actual = value.owned_heap_bytes();
        if actual > budget.retained || actual > limits.maximum_retained_heap_bytes {
            return Err(NativeProfilePreparationRefusal::Capacity);
        }
        Ok((
            value,
            NativeProfilePreparationReceipt {
                preparation_requested_bytes_bound: budget.preparation,
                retained_heap_bytes_bound: budget.retained,
                retained_accounted_heap_bytes: actual,
            },
        ))
    }
}
#[derive(Debug, Clone, Copy)]
pub struct NativeOfferStorageReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
    pub retained_heap_bytes: usize,
}
#[derive(Debug)]
pub enum NativeOfferStorageRefusal {
    Capacity,
    Profile(String),
    Inventory(conduit_core::PlanStorageRefusal),
}
impl PreparedNativeProfile {
    /// This exact Source-authored two-port native admission offer only. It uses
    /// cached exact Core profile KindIDs and exact-capacity string construction.
    pub fn offer_storage_reservation(
        &self,
        flow: bool,
    ) -> Result<NativeOfferStorageReceipt, NativeOfferStorageRefusal> {
        let add = |a: usize, b: usize| a.checked_add(b).ok_or(NativeOfferStorageRefusal::Capacity);
        let mul = |a: usize, b: usize| a.checked_mul(b).ok_or(NativeOfferStorageRefusal::Capacity);
        let identity = add(
            add("structure/".len(), if flow { "flow-".len() } else { 0 })?,
            add("native-profile/".len(), self.identity.len())?,
        )?;
        let implementation = NATIVE_PROFILE_IMPLEMENTATION.len();
        let revision = add(add(implementation, 1)?, identity)?;
        let port_ids = mul(2, add("candidate".len(), "result".len())?)?;
        let value_kinds = mul(
            2,
            add(
                self.candidate_kind.as_str().len(),
                self.native_kind.as_str().len(),
            )?,
        )?;
        let arrays = add(
            add(
                mul(2, size_of::<PortDescriptor>())?,
                mul(2, size_of::<FrontValueContract>())?,
            )?,
            size_of::<KindSemanticLaw>(),
        )?;
        let strings = add(
            add(add(identity, mul(2, revision)?)?, mul(3, implementation)?)?,
            add(port_ids, value_kinds)?,
        )?;
        let retained = add(arrays, strings)?;
        Ok(NativeOfferStorageReceipt {
            preparation_requested_bytes_bound: add(retained, mul(2, identity)?)?,
            retained_heap_bytes_bound: retained,
            retained_heap_bytes: 0,
        })
    }
    pub fn offer_with_storage_limits(
        &self,
        flow: bool,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<(CapabilityOffer, NativeOfferStorageReceipt), NativeOfferStorageRefusal> {
        let mut receipt = self.offer_storage_reservation(flow)?;
        if receipt.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || receipt.retained_heap_bytes_bound > maximum_retained_heap_bytes
        {
            return Err(NativeOfferStorageRefusal::Capacity);
        }
        let offer = self
            .offer(flow)
            .map_err(NativeOfferStorageRefusal::Profile)?;
        receipt.retained_heap_bytes = conduit_core::capability_offer_owned_heap_bytes(&offer)
            .map_err(NativeOfferStorageRefusal::Inventory)?;
        if receipt.retained_heap_bytes > receipt.retained_heap_bytes_bound {
            return Err(NativeOfferStorageRefusal::Capacity);
        }
        Ok((offer, receipt))
    }
}
