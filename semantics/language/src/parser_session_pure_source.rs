//! Fixed pure Source ingress for the closed Session driver. Full generated
//! Native admission surrounds exact prepared Source evaluation. Original
//! canonical programs and complete historical frames remain independently owned.
use alloc::{rc::Rc, vec::Vec};
use conduit_plot::{
    rust_binding::{
        NativeBindingRefusal, NativeFamilyTypeDescriptor, PreparedNativeFamily,
        PreparedNativeRustBinding,
    },
    PortableExpressionProgram, PreparedExpressionStorageReceipt,
    PreparedPortableExpressionEvaluator,
};
use core::{cell::RefCell, mem::size_of};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PureSourceLimits {
    pub(crate) maximum_invocations: u32,
    pub(crate) maximum_history_retained_bytes: usize,
    pub(crate) maximum_input_bytes: usize,
    pub(crate) maximum_output_bytes: usize,
    pub(crate) maximum_program_bytes: usize,
    pub(crate) maximum_program_decode_bytes: usize,
    pub(crate) maximum_type_encoding_bytes: usize,
    pub(crate) maximum_evaluator_retained_bytes: usize,
    pub(crate) maximum_evaluator_preparation_bytes: usize,
    pub(crate) maximum_active_native_bytes: usize,
    pub(crate) other_existing_bytes: usize,
    pub(crate) maximum_combined_preparation_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct PureSourceReservation {
    pub(crate) programs: usize,
    pub(crate) active_native_bytes_bound: usize,
    pub(crate) combined_preparation_bytes_bound: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct PureSourceReceipt {
    pub(crate) original_program_artifact_bytes: usize,
    pub(crate) evaluator: PreparedExpressionStorageReceipt,
    pub(crate) active_native_bytes_bound: usize,
    pub(crate) combined_preparation_bytes_bound: usize,
}
#[derive(Debug)]
pub(crate) enum PureSourceRefusal {
    Closed,
    Pressure,
    Descriptor,
    Program,
    Type,
    Native(NativeBindingRefusal),
    Output,
}
type Readmit = fn(&mut PreparedNativeFamily, &[u8]) -> Result<(), NativeBindingRefusal>;
fn readmit<T: PreparedNativeRustBinding>(
    family: &mut PreparedNativeFamily,
    bytes: &[u8],
) -> Result<(), NativeBindingRefusal> {
    drop(family.decode::<T>(bytes)?);
    Ok(())
}
pub(crate) struct PureSourceFrames {
    pub(crate) input: Vec<u8>,
    pub(crate) output: Vec<u8>,
    pub(crate) intermediates: Vec<Vec<u8>>,
}
pub(crate) struct PureSourceHistory {
    pub(crate) ordinal: u64,
    pub(crate) input: Vec<u8>,
    pub(crate) output: Vec<u8>,
    pub(crate) intermediates: Vec<Vec<u8>>,
    original_program: &'static str,
    input_descriptor: &'static NativeFamilyTypeDescriptor,
    output_descriptor: &'static NativeFamilyTypeDescriptor,
}
pub(crate) struct PreparedParserPureSource {
    original_program: &'static str,
    evaluators: Vec<PreparedPortableExpressionEvaluator>,
    input_family: Rc<RefCell<PreparedNativeFamily>>,
    output_family: Rc<RefCell<PreparedNativeFamily>>,
    input_descriptor: &'static NativeFamilyTypeDescriptor,
    output_descriptor: &'static NativeFamilyTypeDescriptor,
    input_readmit: Readmit,
    output_readmit: Readmit,
    limits: PureSourceLimits,
    pub(crate) receipt: PureSourceReceipt,
    next_ordinal: u64,
    closed: bool,
}
fn add(a: usize, b: usize) -> Result<usize, PureSourceRefusal> {
    a.checked_add(b).ok_or(PureSourceRefusal::Pressure)
}
impl PreparedParserPureSource {
    /// Library-owned bindings supply the fixed generated program, never caller
    /// programs or individually Native-valid snapshots. Families are already
    /// prepared owners; their own construction and static schemas are additional
    /// reservations in the enclosing whole-profile factory.
    pub(crate) fn prepare<I: PreparedNativeRustBinding, O: PreparedNativeRustBinding>(
        original_program: &'static str,
        input_family: Rc<RefCell<PreparedNativeFamily>>,
        output_family: Rc<RefCell<PreparedNativeFamily>>,
        limits: PureSourceLimits,
    ) -> Result<Self, PureSourceRefusal> {
        use PureSourceRefusal as R;
        let reservation =
            Self::reservation::<I, O>(original_program, &input_family, &output_family, limits)?;
        let count = reservation.programs;
        let active = reservation.active_native_bytes_bound;
        let combined = reservation.combined_preparation_bytes_bound;
        let mut evaluators = Vec::new();
        evaluators
            .try_reserve_exact(count)
            .map_err(|_| R::Pressure)?;
        if evaluators.capacity() != count {
            return Err(R::Pressure);
        }
        let mut prior_output: Option<Vec<u8>> = None;
        let mut evaluator_receipt = PreparedExpressionStorageReceipt {
            decoded_program_heap_bytes: 0,
            preparation_requested_bytes_bound: 0,
            retained_heap_bytes_bound: 0,
        };
        for (position, encoded) in original_program.lines().enumerate() {
            let length = encoded.len() / 2;
            let mut raw = Vec::new();
            raw.try_reserve_exact(length).map_err(|_| R::Pressure)?;
            if raw.capacity() != length {
                return Err(R::Pressure);
            }
            for pair in encoded.as_bytes().chunks_exact(2) {
                let nibble = |v: u8| -> u8 {
                    match v {
                        b'0'..=b'9' => v - b'0',
                        b'a'..=b'f' => v - b'a' + 10,
                        b'A'..=b'F' => v - b'A' + 10,
                        _ => unreachable!(),
                    }
                };
                raw.push((nibble(pair[0]) << 4) | nibble(pair[1]));
            }
            let decode = PortableExpressionProgram::canonical_decode_storage_bound(&raw)
                .map_err(|_| R::Program)?;
            if decode > limits.maximum_program_decode_bytes {
                return Err(R::Pressure);
            }
            let program =
                PortableExpressionProgram::from_canonical_bytes_with_storage_limit(&raw, decode)
                    .map_err(|_| R::Program)?;
            if program
                .input_type
                .canonical_byte_length()
                .map_err(|_| R::Type)?
                > limits.maximum_type_encoding_bytes
                || program
                    .output_type
                    .canonical_byte_length()
                    .map_err(|_| R::Type)?
                    > limits.maximum_type_encoding_bytes
            {
                return Err(R::Pressure);
            }
            {
                let input = program.input_type.canonical_bytes().map_err(|_| R::Type)?;
                let expected = prior_output
                    .as_deref()
                    .unwrap_or(I::PREPARED_DESCRIPTOR.type_bytes);
                if input.as_slice() != expected {
                    return Err(R::Type);
                }
            }
            drop(prior_output.take());
            let output = program.output_type.canonical_bytes().map_err(|_| R::Type)?;
            if position + 1 == count && output.as_slice() != O::PREPARED_DESCRIPTOR.type_bytes {
                return Err(R::Type);
            }
            prior_output = Some(output);
            let (evaluator, receipt) =
                PreparedPortableExpressionEvaluator::new_with_storage_limits(
                    &program,
                    decode,
                    limits.maximum_evaluator_preparation_bytes,
                    limits.maximum_evaluator_retained_bytes,
                )
                .map_err(|_| R::Pressure)?;
            if evaluator.output_capacity() > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                || (position + 1 == count
                    && evaluator.output_capacity() > limits.maximum_output_bytes)
            {
                return Err(R::Pressure);
            }
            evaluator_receipt.decoded_program_heap_bytes = add(
                evaluator_receipt.decoded_program_heap_bytes,
                receipt.decoded_program_heap_bytes,
            )?;
            evaluator_receipt.preparation_requested_bytes_bound = add(
                evaluator_receipt.preparation_requested_bytes_bound,
                receipt.preparation_requested_bytes_bound,
            )?;
            evaluator_receipt.retained_heap_bytes_bound = add(
                evaluator_receipt.retained_heap_bytes_bound,
                receipt.retained_heap_bytes_bound,
            )?;
            evaluators.push(evaluator);
        }
        Ok(Self {
            original_program,
            evaluators,
            input_family,
            output_family,
            input_descriptor: I::PREPARED_DESCRIPTOR,
            output_descriptor: O::PREPARED_DESCRIPTOR,
            input_readmit: readmit::<I>,
            output_readmit: readmit::<O>,
            limits,
            receipt: PureSourceReceipt {
                original_program_artifact_bytes: original_program.len(),
                evaluator: evaluator_receipt,
                active_native_bytes_bound: active,
                combined_preparation_bytes_bound: combined,
            },
            next_ordinal: 0,
            closed: false,
        })
    }
    /// Allocation-free readiness and whole-port reservation for the enclosing
    /// bank: it sums every selected port before constructing the first one.
    pub(crate) fn reservation<I: PreparedNativeRustBinding, O: PreparedNativeRustBinding>(
        original_program: &'static str,
        input_family: &Rc<RefCell<PreparedNativeFamily>>,
        output_family: &Rc<RefCell<PreparedNativeFamily>>,
        limits: PureSourceLimits,
    ) -> Result<PureSourceReservation, PureSourceRefusal> {
        use PureSourceRefusal as R;
        let input = input_family.try_borrow().map_err(|_| R::Closed)?;
        let output = output_family.try_borrow().map_err(|_| R::Closed)?;
        if !input.contains_descriptor(I::PREPARED_DESCRIPTOR)
            || !output.contains_descriptor(O::PREPARED_DESCRIPTOR)
        {
            return Err(R::Descriptor);
        }
        let input_receipt = input.storage_receipt();
        let output_receipt = output.storage_receipt();
        let mut family_bytes = input_receipt.retained_heap_bytes_bound;
        if !Rc::ptr_eq(input_family, output_family) {
            family_bytes = add(family_bytes, output_receipt.retained_heap_bytes_bound)?;
        }
        let active = input_receipt
            .conversion_requested_bytes_bound
            .max(output_receipt.conversion_requested_bytes_bound);
        drop(input);
        drop(output);
        let count = original_program.lines().count();
        if limits.maximum_invocations == 0
            || limits.maximum_input_bytes == 0
            || limits.maximum_output_bytes == 0
            || limits.maximum_input_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || limits.maximum_output_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || active > limits.maximum_active_native_bytes
            || !(1..=64).contains(&count)
        {
            return Err(R::Pressure);
        }
        let mut raw_length = 0;
        for encoded in original_program.lines() {
            if encoded.is_empty()
                || !encoded.len().is_multiple_of(2)
                || !encoded.bytes().all(|v| v.is_ascii_hexdigit())
            {
                return Err(R::Program);
            }
            raw_length = raw_length.max(encoded.len() / 2);
        }
        if raw_length == 0 || raw_length > limits.maximum_program_bytes {
            return Err(R::Pressure);
        }
        // Every ordered Source gear is retained. Reserve the complete chain
        // before even its evaluator array or the first hex buffer is allocated.
        let retained = limits
            .maximum_evaluator_retained_bytes
            .checked_mul(count)
            .ok_or(R::Pressure)?;
        let array = size_of::<PreparedPortableExpressionEvaluator>()
            .checked_mul(count)
            .ok_or(R::Pressure)?;
        let preparation = add(
            add(
                limits.maximum_program_decode_bytes,
                limits.maximum_evaluator_preparation_bytes,
            )?,
            limits
                .maximum_type_encoding_bytes
                .checked_mul(2)
                .ok_or(R::Pressure)?,
        )?;
        let source_preparation = add(add(add(raw_length, preparation)?, retained)?, array)?;
        let source_execution = add(add(retained, array)?, active)?;
        let histories = limits
            .maximum_history_retained_bytes
            .checked_mul(limits.maximum_invocations as usize)
            .ok_or(R::Pressure)?;
        let combined = add(
            add(
                add(size_of::<Self>(), family_bytes)?,
                add(
                    original_program.len(),
                    add(histories, source_preparation.max(source_execution))?,
                )?,
            )?,
            limits.other_existing_bytes,
        )?;
        if combined > limits.maximum_combined_preparation_bytes {
            return Err(R::Pressure);
        }
        Ok(PureSourceReservation {
            programs: count,
            active_native_bytes_bound: active,
            combined_preparation_bytes_bound: combined,
        })
    }
    /// Actual finite buffers already proven by exact prepared Source. The bank
    /// reserves the complete history pool before constructing these buffers.
    pub(crate) fn intermediate_capacities(&self) -> impl Iterator<Item = usize> + '_ {
        self.evaluators[..self.evaluators.len() - 1]
            .iter()
            .map(|e| e.output_capacity())
    }
    pub(crate) fn cancel(&mut self) {
        self.closed = true;
    }
    pub(crate) fn execute(
        &mut self,
        query: &[u8],
        mut frames: PureSourceFrames,
    ) -> Result<PureSourceHistory, PureSourceRefusal> {
        use PureSourceRefusal as R;
        if self.closed {
            return Err(R::Closed);
        }
        // Every failure below closes this consumed port. The enclosing atomic
        // revision additionally cancels all Source/model ingresses on refusal.
        self.closed = true;
        let mut history_bytes = add(size_of::<PureSourceHistory>(), frames.input.capacity())?;
        history_bytes = add(history_bytes, frames.output.capacity())?;
        history_bytes = add(
            history_bytes,
            frames
                .intermediates
                .capacity()
                .checked_mul(size_of::<Vec<u8>>())
                .ok_or(R::Pressure)?,
        )?;
        for frame in &frames.intermediates {
            history_bytes = add(history_bytes, frame.capacity())?;
        }
        if history_bytes > self.limits.maximum_history_retained_bytes {
            return Err(R::Pressure);
        }
        if self.next_ordinal >= u64::from(self.limits.maximum_invocations)
            || query.len() > self.limits.maximum_input_bytes
            || frames.input.capacity() < self.limits.maximum_input_bytes
            || frames.output.capacity() < self.limits.maximum_output_bytes
            || frames.intermediates.len() + 1 != self.evaluators.len()
            || frames
                .intermediates
                .iter()
                .zip(&self.evaluators)
                .any(|(frame, evaluator)| frame.capacity() < evaluator.output_capacity())
        {
            return Err(R::Pressure);
        }
        frames.input.clear();
        frames.input.extend_from_slice(query);
        (self.input_readmit)(
            &mut *self.input_family.try_borrow_mut().map_err(|_| R::Closed)?,
            &frames.input,
        )
        .map_err(R::Native)?;
        let expected = evaluate_chain(
            &mut self.evaluators,
            &frames.input,
            &mut frames.intermediates,
        )?;
        if expected.len() > self.limits.maximum_output_bytes {
            return Err(R::Output);
        }
        (self.output_readmit)(
            &mut *self.output_family.try_borrow_mut().map_err(|_| R::Closed)?,
            expected,
        )
        .map_err(R::Native)?;
        frames.output.clear();
        frames.output.extend_from_slice(expected);
        let history = PureSourceHistory {
            ordinal: self.next_ordinal,
            input: frames.input,
            output: frames.output,
            intermediates: frames.intermediates,
            original_program: self.original_program,
            input_descriptor: self.input_descriptor,
            output_descriptor: self.output_descriptor,
        };
        self.next_ordinal += 1;
        self.closed = false;
        Ok(history)
    }
    /// Independently budgeted historical readmission reuses this exact prepared
    /// Source owner, with no target call or free ordinary Native decoding.
    pub(crate) fn replay(&mut self, history: &PureSourceHistory) -> Result<(), PureSourceRefusal> {
        use PureSourceRefusal as R;
        let result = (|| {
            if !core::ptr::eq(history.original_program, self.original_program)
                || !core::ptr::eq(history.input_descriptor, self.input_descriptor)
                || !core::ptr::eq(history.output_descriptor, self.output_descriptor)
                || history.ordinal >= self.next_ordinal
                || history.input.len() > self.limits.maximum_input_bytes
                || history.output.len() > self.limits.maximum_output_bytes
            {
                return Err(R::Program);
            }
            (self.input_readmit)(
                &mut *self.input_family.try_borrow_mut().map_err(|_| R::Closed)?,
                &history.input,
            )
            .map_err(R::Native)?;
            let expected =
                verify_chain(&mut self.evaluators, &history.input, &history.intermediates)?;
            if expected != history.output.as_slice() {
                return Err(R::Output);
            }
            (self.output_readmit)(
                &mut *self.output_family.try_borrow_mut().map_err(|_| R::Closed)?,
                &history.output,
            )
            .map_err(R::Native)
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
}

fn evaluate_chain<'a>(
    evaluators: &'a mut [PreparedPortableExpressionEvaluator],
    input: &[u8],
    intermediates: &mut [Vec<u8>],
) -> Result<&'a [u8], PureSourceRefusal> {
    let (first, rest) = evaluators
        .split_first_mut()
        .ok_or(PureSourceRefusal::Program)?;
    let output = first
        .evaluate(input)
        .map_err(|_| PureSourceRefusal::Program)?;
    if rest.is_empty() {
        if !intermediates.is_empty() {
            return Err(PureSourceRefusal::Program);
        }
        Ok(output)
    } else {
        let (frame, remaining) = intermediates
            .split_first_mut()
            .ok_or(PureSourceRefusal::Program)?;
        if output.len() > frame.capacity() {
            return Err(PureSourceRefusal::Pressure);
        }
        frame.clear();
        frame.extend_from_slice(output);
        evaluate_chain(rest, output, remaining)
    }
}
fn verify_chain<'a>(
    evaluators: &'a mut [PreparedPortableExpressionEvaluator],
    input: &[u8],
    intermediates: &[Vec<u8>],
) -> Result<&'a [u8], PureSourceRefusal> {
    let (first, rest) = evaluators
        .split_first_mut()
        .ok_or(PureSourceRefusal::Program)?;
    let output = first
        .evaluate(input)
        .map_err(|_| PureSourceRefusal::Program)?;
    if rest.is_empty() {
        if !intermediates.is_empty() {
            return Err(PureSourceRefusal::Program);
        }
        Ok(output)
    } else {
        let (frame, remaining) = intermediates
            .split_first()
            .ok_or(PureSourceRefusal::Program)?;
        if output != frame.as_slice() {
            return Err(PureSourceRefusal::Output);
        }
        verify_chain(rest, output, remaining)
    }
}
