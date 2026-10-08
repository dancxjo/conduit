// Optional exact Source evaluation preparation. Ordinary Native input
// construction/encoding and caller-owned outputs are outside this receipt.
use conduit_plot::{PreparedExpressionStorageReceipt, PreparedPortableExpressionEvaluator};
use core::mem::size_of;

#[derive(Clone, Copy, Debug)]
pub struct Window8SourcePreparationLimits {
    pub maximum_retained_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
    pub maximum_input_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct Window8PreparedSourceReceipt {
    /// Includes exact prepared Native family and every prepared Source evaluator.
    pub retained_heap_bytes_bound: usize,
    pub preparation_peak_heap_bytes_bound: usize,
    pub programs: usize,
    pub native: PreparedNativeFamilyStorageReceipt,
}
struct PreparedBankProgram {
    name: &'static str,
    evaluator: RefCell<PreparedPortableExpressionEvaluator>,
    receipt: PreparedExpressionStorageReceipt,
}
fn plus(a: usize, b: usize) -> Result<usize, Window8PreparedBankRefusal> {
    a.checked_add(b)
        .ok_or(Window8PreparedBankRefusal::SourceCapacity)
}
fn ceiling(value: usize, maximum: usize) -> Result<(), Window8PreparedBankRefusal> {
    if value > maximum {
        Err(Window8PreparedBankRefusal::SourceCapacity)
    } else {
        Ok(())
    }
}
impl Window8ProgramBank {
    /// Refuses unsupported Source preparation before returning the bank. There
    /// is no runtime fallback to another evaluator or weakened Native admission.
    /// The default `prepare`/`prepare_native` Reference modes remain unchanged.
    pub fn prepare_native_evaluator(
        native_limits: PreparedNativeFamilyLimits,
        limits: Window8SourcePreparationLimits,
    ) -> Result<Self, Window8PreparedBankRefusal> {
        Self::prepare_evaluator_bank(
            native_limits,
            limits,
            prepare_native_family,
            source_entries(),
        )
    }
    fn prepare_evaluator_bank<const N: usize>(
        mut native_limits: PreparedNativeFamilyLimits,
        limits: Window8SourcePreparationLimits,
        prepare_family: fn(
            PreparedNativeFamilyLimits,
        ) -> Result<PreparedNativeFamily, PreparedNativeFamilyRefusal>,
        entries: [(&'static str, &'static str); N],
    ) -> Result<Self, Window8PreparedBankRefusal> {
        if limits.maximum_input_bytes == 0
            || limits.maximum_input_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(Window8PreparedBankRefusal::SourceCapacity);
        }
        // Prepare the exact same complete Native output closure as the existing
        // explicit Native mode. This helper does not decode any Source programs.
        let owner = size_of::<Self>();
        native_limits.maximum_retained_bytes = native_limits.maximum_retained_bytes.min(
            limits
                .maximum_retained_bytes
                .checked_sub(owner)
                .ok_or(Window8PreparedBankRefusal::SourceCapacity)?,
        );
        native_limits.maximum_preparation_peak_bytes =
            native_limits.maximum_preparation_peak_bytes.min(
                limits
                    .maximum_preparation_peak_bytes
                    .checked_sub(owner)
                    .ok_or(Window8PreparedBankRefusal::SourceCapacity)?,
            );
        let family =
            prepare_family(native_limits).map_err(Window8PreparedBankRefusal::NativePreparation)?;
        let native = family.storage_receipt();
        let slots = entries
            .len()
            .checked_mul(size_of::<PreparedBankProgram>())
            .ok_or(Window8PreparedBankRefusal::SourceCapacity)?;
        let mut retained = plus(
            plus(size_of::<Self>(), native.retained_heap_bytes_bound)?,
            slots,
        )?;
        ceiling(retained, limits.maximum_retained_bytes)?;
        let mut peak =
            plus(size_of::<Self>(), native.preparation_peak_heap_bytes_bound)?.max(retained);
        ceiling(peak, limits.maximum_preparation_peak_bytes)?;
        let mut prepared = Vec::with_capacity(entries.len());
        for (name, encoded) in entries {
            if !encoded.len().is_multiple_of(2) {
                return Err(Window8PreparedBankRefusal::SourcePreparation(
                    Window8Refusal::Program,
                ));
            }
            let raw_length = encoded.len() / 2;
            ceiling(
                plus(retained, raw_length)?,
                limits.maximum_preparation_peak_bytes,
            )?;
            let mut raw = Vec::with_capacity(raw_length);
            for position in (0..encoded.len()).step_by(2) {
                raw.push(
                    u8::from_str_radix(&encoded[position..position + 2], 16).map_err(|_| {
                        Window8PreparedBankRefusal::SourcePreparation(Window8Refusal::Program)
                    })?,
                )
            }
            let decode =
                PortableExpressionProgram::canonical_decode_storage_bound(&raw).map_err(|_| {
                    Window8PreparedBankRefusal::SourcePreparation(Window8Refusal::Program)
                })?;
            let base = plus(retained, raw.capacity())?;
            let decoded_peak = plus(base, decode)?;
            ceiling(decoded_peak, limits.maximum_preparation_peak_bytes)?;
            peak = peak.max(decoded_peak);
            let program =
                PortableExpressionProgram::from_canonical_bytes_with_storage_limit(&raw, decode)
                    .map_err(|_| {
                        Window8PreparedBankRefusal::SourcePreparation(Window8Refusal::Program)
                    })?;
            let (evaluator, receipt) =
                PreparedPortableExpressionEvaluator::new_with_storage_limits(
                    &program,
                    decode,
                    limits.maximum_preparation_peak_bytes - base,
                    limits.maximum_retained_bytes - retained,
                )
                .map_err(Window8PreparedBankRefusal::SourceEvaluator)?;
            peak = peak.max(plus(
                plus(base, receipt.decoded_program_heap_bytes)?,
                receipt.preparation_requested_bytes_bound,
            )?);
            retained = plus(retained, receipt.retained_heap_bytes_bound)?;
            ceiling(retained, limits.maximum_retained_bytes)?;
            ceiling(peak, limits.maximum_preparation_peak_bytes)?;
            prepared.push(PreparedBankProgram {
                name,
                evaluator: RefCell::new(evaluator),
                receipt,
            });
        }
        let receipt = Window8PreparedSourceReceipt {
            retained_heap_bytes_bound: retained,
            preparation_peak_heap_bytes_bound: peak,
            programs: prepared.len(),
            native,
        };
        Ok(Self {
            programs: BTreeMap::new(),
            native: Some(RefCell::new(family)),
            prepared: Some(prepared),
            prepared_receipt: Some(receipt),
            maximum_prepared_input_bytes: limits.maximum_input_bytes,
        })
    }
    pub fn prepared_source_receipt(&self) -> Option<Window8PreparedSourceReceipt> {
        self.prepared_receipt
    }
    pub fn prepared_program_receipts(
        &self,
    ) -> impl Iterator<Item = (&'static str, PreparedExpressionStorageReceipt)> + '_ {
        self.prepared.iter().flat_map(|programs| {
            programs
                .iter()
                .map(|program| (program.name, program.receipt))
        })
    }
}

#[cfg(test)]
mod prepared_source_frame_tests {
    use super::*;
    #[test]
    fn every_exact_program_refuses_malformed_and_foreign_complete_inputs() {
        let foreign = conduit_core::StructuredInfoValue::leaf(
            conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap(),
            0_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        for (name, encoded) in proposal_source_entries() {
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            let (mut prepared, _) = PreparedPortableExpressionEvaluator::new_with_storage_limits(
                &program,
                usize::MAX,
                1024 * 1024 * 1024,
                1024 * 1024 * 1024,
            )
            .unwrap();
            for bytes in [&[][..], &b"malformed"[..], foreign.as_slice()] {
                assert!(program.evaluate(bytes).is_err(), "{name}");
                assert!(prepared.evaluate(bytes).is_err(), "{name}");
            }
        }
    }
}
