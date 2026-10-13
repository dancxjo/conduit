//! Fixed pure composition admission. This evaluates only the checked linear
//! expression composition; it owns no scheduling, effects or parser policy.
use super::{ParserSessionEntry, ParserSessionVerificationLimits};
use alloc::vec::Vec;
use conduit_core::StructuredInfoType;
use conduit_plot::{
    PortableExpressionProgram, PreparedExpressionStorageRefusal,
    PreparedPortableExpressionEvaluator,
};

/// Bounds canonical decoding and cumulative requested preparation allocations,
/// including temporary program bytes and separately returned endpoint Type owners.
/// Native values, target execution and Flow storage require separate admission.
#[derive(Clone, Copy, Debug)]
pub struct ParserSessionVerificationReceipt {
    pub decoded_program_heap_bytes_bound: usize,
    pub preparation_peak_heap_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
}
#[derive(Debug)]
pub(crate) enum VerificationRefusal {
    Program,
    Ports,
    Storage(PreparedExpressionStorageRefusal),
}
pub(crate) struct PreparedSourceVerification {
    entry: ParserSessionEntry,
    evaluators: Vec<PreparedPortableExpressionEvaluator>,
}
impl PreparedSourceVerification {
    pub(crate) fn prepare(
        entry: ParserSessionEntry,
        limits: ParserSessionVerificationLimits,
    ) -> Result<
        (
            Self,
            StructuredInfoType,
            StructuredInfoType,
            ParserSessionVerificationReceipt,
        ),
        VerificationRefusal,
    > {
        use VerificationRefusal as R;
        let count = entry.program_hex().lines().count();
        if !(1..=64).contains(&count) {
            return Err(R::Program);
        }
        let vector_bytes = count
            .checked_mul(core::mem::size_of::<PreparedPortableExpressionEvaluator>())
            .ok_or(R::Program)?;
        if vector_bytes > limits.retained_bytes || vector_bytes > limits.preparation_peak_bytes {
            return Err(R::Storage(PreparedExpressionStorageRefusal::Capacity));
        }
        let mut evaluators = Vec::new();
        evaluators
            .try_reserve_exact(count)
            .map_err(|_| R::Storage(PreparedExpressionStorageRefusal::Capacity))?;
        let mut retained = vector_bytes;
        let mut preparation = vector_bytes;
        let mut decoded = 0usize;
        let mut input_type = None;
        let mut previous_output = None;
        for hex in entry.program_hex().lines() {
            // Admit the fixed canonical byte buffer before even decoding hex.
            let byte_length = hex.len().checked_div(2).ok_or(R::Program)?;
            if hex.len() % 2 != 0
                || byte_length > conduit_plot::MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES
            {
                return Err(R::Program);
            }
            let available = limits
                .preparation_peak_bytes
                .checked_sub(preparation)
                .ok_or(R::Program)?;
            if byte_length > available {
                return Err(R::Storage(PreparedExpressionStorageRefusal::Capacity));
            }
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(byte_length)
                .map_err(|_| R::Storage(PreparedExpressionStorageRefusal::Capacity))?;
            let byte_capacity = bytes.capacity();
            if byte_capacity > available {
                return Err(R::Storage(PreparedExpressionStorageRefusal::Capacity));
            }
            fn nibble(byte: u8) -> Result<u8, VerificationRefusal> {
                match byte {
                    b'0'..=b'9' => Ok(byte - b'0'),
                    b'a'..=b'f' => Ok(byte - b'a' + 10),
                    _ => Err(VerificationRefusal::Program),
                }
            }
            for pair in hex.as_bytes().chunks_exact(2) {
                bytes.push((nibble(pair[0])? << 4) | nibble(pair[1])?);
            }
            // This scan allocates nothing. The original canonical decoder still
            // enforces complete identity after its entire request ceiling is admitted.
            let decode_bound = PortableExpressionProgram::canonical_decode_storage_bound(&bytes)
                .map_err(|_| R::Program)?;
            let parse_peak = byte_capacity.checked_add(decode_bound).ok_or(R::Program)?;
            if parse_peak > available
                || decode_bound
                    > limits
                        .decoded_program_bytes
                        .checked_sub(decoded)
                        .ok_or(R::Program)?
            {
                return Err(R::Storage(PreparedExpressionStorageRefusal::Capacity));
            }
            preparation = preparation.checked_add(parse_peak).ok_or(R::Program)?;
            let program = PortableExpressionProgram::from_canonical_bytes_with_storage_limit(
                &bytes,
                decode_bound,
            )
            .map_err(|_| R::Program)?;
            decoded = decoded.checked_add(decode_bound).ok_or(R::Program)?;
            // This overcounts the full decoded AST for each endpoint; it also
            // charges both returned full Type owners after the AST is dropped.
            retained = retained.checked_add(decode_bound).ok_or(R::Program)?;
            if retained > limits.retained_bytes {
                return Err(R::Storage(PreparedExpressionStorageRefusal::Capacity));
            }
            if previous_output
                .as_ref()
                .is_some_and(|t| t != &program.input_type)
            {
                return Err(R::Ports);
            }
            let (evaluator, receipt) =
                PreparedPortableExpressionEvaluator::new_with_storage_limits(
                    &program,
                    decode_bound,
                    limits
                        .preparation_peak_bytes
                        .checked_sub(preparation)
                        .ok_or(R::Program)?,
                    limits
                        .retained_bytes
                        .checked_sub(retained)
                        .ok_or(R::Program)?,
                )
                .map_err(R::Storage)?;
            preparation = preparation
                .checked_add(receipt.decoded_program_heap_bytes)
                .and_then(|n| n.checked_add(receipt.preparation_requested_bytes_bound))
                .ok_or(R::Program)?;
            retained = retained
                .checked_add(receipt.retained_heap_bytes_bound)
                .ok_or(R::Program)?;
            if input_type.is_none() {
                input_type = Some(program.input_type);
            }
            previous_output = Some(program.output_type);
            evaluators.push(evaluator);
        }
        Ok((
            Self { entry, evaluators },
            input_type.ok_or(R::Program)?,
            previous_output.ok_or(R::Program)?,
            ParserSessionVerificationReceipt {
                decoded_program_heap_bytes_bound: decoded,
                preparation_peak_heap_bytes_bound: preparation,
                retained_heap_bytes_bound: retained,
            },
        ))
    }
    pub(crate) fn entry(&self) -> ParserSessionEntry {
        self.entry
    }
    pub(crate) fn evaluate<'a>(&'a mut self, input: &'a [u8]) -> Result<&'a [u8], ()> {
        let mut value = input;
        for evaluator in &mut self.evaluators {
            value = evaluator.evaluate(value).map_err(|_| ())?;
        }
        Ok(value)
    }
}
