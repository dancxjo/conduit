//! Actual Source-law guards within the existing selected-node interpreter.
use super::*;
use crate::rust_binding::{
    NativeLawRegistry, PreparedNativeBounds, PreparedNativeCapacityReport, PreparedNativeRefusal,
    PreparedRegisteredValue,
};
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Shared;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc as Shared;
use conduit_core::StructuredInfoType;

/// Aggregate prepared guard limits, separate from Source/metadata custody.
/// Native counts and byte capacities apply across all guards, not per AST node.
#[derive(Clone, Copy, Debug)]
pub struct PreparedExpressionLawBounds {
    pub guards: usize,
    pub native: PreparedNativeBounds,
    pub occurrences_per_guard: usize,
    pub steps: usize,
}
/// Actual prepared counts and conservative Native-admission step bound.
/// Byte capacities exclude metadata, path/index storage and general heap usage.
#[derive(Clone, Copy, Debug)]
pub struct PreparedExpressionLawCapacityReport {
    pub guards: usize,
    pub canonical_buffer_capacity: usize,
    pub scopes: usize,
    pub evaluators: usize,
    pub contracts: usize,
    pub admission_step_bound: usize,
}
#[cfg(target_has_atomic = "ptr")]
struct Counter(core::sync::atomic::AtomicUsize);
#[cfg(not(target_has_atomic = "ptr"))]
struct Counter(core::cell::Cell<usize>);
impl Counter {
    fn new(value: usize) -> Self {
        Self(value.into())
    }
    fn get(&self) -> usize {
        #[cfg(target_has_atomic = "ptr")]
        {
            self.0.load(core::sync::atomic::Ordering::Relaxed)
        }
        #[cfg(not(target_has_atomic = "ptr"))]
        {
            self.0.get()
        }
    }
    fn set(&self, value: usize) {
        #[cfg(target_has_atomic = "ptr")]
        {
            self.0.store(value, core::sync::atomic::Ordering::Relaxed);
        }
        #[cfg(not(target_has_atomic = "ptr"))]
        {
            self.0.set(value);
        }
    }
}
struct ContextData {
    registry: NativeLawRegistry,
    bounds: PreparedExpressionLawBounds,
    // Preparation is single-threaded and this private context cannot escape
    // construction until complete. Atomics retain the hosted evaluator's Send.
    used: [Counter; 6],
    terminal: Counter,
}
#[derive(Clone)]
pub(super) struct Context(Shared<ContextData>);
impl Context {
    fn new(
        registry: &NativeLawRegistry,
        bounds: PreparedExpressionLawBounds,
    ) -> Result<Self, Refusal> {
        let n = bounds.native;
        if bounds.guards == 0
            || bounds.guards > 4096
            || n.canonical_bytes == 0
            || n.canonical_bytes > 4 * 1024 * 1024
            || n.scopes > 4096
            || n.invariants > 4096
            || n.contracts > 4096
            || bounds.occurrences_per_guard == 0
            || bounds.occurrences_per_guard > 4096
            || bounds.steps == 0
            || bounds.steps > 16 * 1024 * 1024
        {
            return Err(Refusal::NativeLaw(PreparedNativeRefusal::Bounds));
        }
        Ok(Self(Shared::new(ContextData {
            registry: registry.clone(),
            bounds,
            used: core::array::from_fn(|_| Counter::new(0)),
            terminal: Counter::new(0),
        })))
    }
    pub(super) fn ensure_live(&self) -> Result<(), Refusal> {
        match self.0.terminal.get() {
            0 => Ok(()),
            1 => Err(Refusal::NativeLaw(PreparedNativeRefusal::Cancelled)),
            _ => Err(Refusal::NativeLaw(PreparedNativeRefusal::State)),
        }
    }
    pub(super) fn fail(&self) {
        if self.0.terminal.get() == 0 {
            self.0.terminal.set(2);
        }
    }
    fn charge(&self, report: PreparedNativeCapacityReport, frame: usize) -> Result<(), Refusal> {
        let add = |a: usize, b: usize| {
            a.checked_add(b)
                .ok_or(Refusal::NativeLaw(PreparedNativeRefusal::Bounds))
        };
        let bytes = add(
            add(
                report.canonical_input_capacity,
                report.canonical_scratch_capacity,
            )?,
            add(report.evaluator_output_capacity, frame)?,
        )?;
        let per_occurrence = add(
            add(report.prepared_scopes, report.prepared_evaluators)?,
            add(report.prepared_contracts, 1)?,
        )?;
        let steps = report
            .occurrence_limit
            .checked_mul(per_occurrence)
            .ok_or(Refusal::NativeLaw(PreparedNativeRefusal::Bounds))?;
        let charges = [
            1,
            bytes,
            report.prepared_scopes,
            report.prepared_evaluators,
            report.prepared_contracts,
            steps,
        ];
        let b = self.0.bounds;
        let limits = [
            b.guards,
            b.native.canonical_bytes,
            b.native.scopes,
            b.native.invariants,
            b.native.contracts,
            b.steps,
        ];
        let mut next = [0; 6];
        for i in 0..6 {
            next[i] = add(self.0.used[i].get(), charges[i])?;
            if next[i] > limits[i] {
                return Err(Refusal::NativeLaw(PreparedNativeRefusal::Bounds));
            }
        }
        for (counter, value) in self.0.used.iter().zip(next) {
            counter.set(value);
        }
        Ok(())
    }
}
pub(super) fn context(input: &PreparedInput) -> Option<&Context> {
    match input {
        PreparedInput::Primitive { laws, .. } | PreparedInput::Structured(_, _, _, laws) => {
            laws.as_ref()
        }
    }
}
pub(super) struct NodeGuard {
    context: Context,
    admission: PreparedRegisteredValue,
    prefix: Vec<u8>,
    frame: Vec<u8>,
}
impl NodeGuard {
    fn new(
        ty: &StructuredInfoType,
        capacity: usize,
        primitive: bool,
        context: &Context,
    ) -> Result<Option<Box<Self>>, Refusal> {
        if matches!(ty.shape(), StructuredInfoTypeShape::Leaf(_)) {
            return Ok(None);
        }
        let prefix = if primitive {
            ty.canonical_bytes().map_err(|_| Refusal::InvalidProgram)?
        } else {
            Vec::new()
        };
        let capacity = if primitive {
            capacity
                .checked_add(prefix.len())
                .and_then(|v| v.checked_add(5))
                .ok_or(Refusal::NativeLaw(PreparedNativeRefusal::Bounds))?
        } else {
            capacity
        };
        if capacity > context.0.bounds.native.canonical_bytes {
            return Err(Refusal::NativeLaw(PreparedNativeRefusal::Bounds));
        }
        let mut bounds = context.0.bounds.native;
        bounds.canonical_bytes = capacity;
        let admission = context
            .0
            .registry
            .prepare_type(ty, bounds, context.0.bounds.occurrences_per_guard)
            .map_err(Refusal::NativeLaw)?;
        // Each individual allocation is bounded before preparation. Charge the
        // actual retained capacities before this guard can enter the evaluator;
        // one rejected candidate is the maximum transient preparation excess.
        let frame = Vec::with_capacity(if primitive { capacity } else { 0 });
        let framing = frame
            .capacity()
            .checked_add(prefix.capacity())
            .ok_or(Refusal::NativeLaw(PreparedNativeRefusal::Bounds))?;
        context.charge(admission.capacity_report(), framing)?;
        Ok(Some(Box::new(Self {
            context: context.clone(),
            admission,
            prefix,
            frame,
        })))
    }
    pub(super) fn canonical(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        self.context.ensure_live()?;
        let result = self
            .admission
            .check_value(bytes)
            .map_err(Refusal::NativeLaw);
        if result.is_err() {
            self.context.fail();
        }
        result
    }
    pub(super) fn primitive(&mut self, value: &PrimitiveValue<'_>) -> Result<(), Refusal> {
        self.frame.clear();
        nominal::append_output(&mut self.frame, &self.prefix, value)?;
        self.context.ensure_live()?;
        let result = self
            .admission
            .check_value(&self.frame)
            .map_err(Refusal::NativeLaw);
        if result.is_err() {
            self.context.fail();
        }
        result
    }
}
pub(super) fn primitive_guard(
    node: &PortableExpressionNode,
    input: &StructuredInfoType,
    prepared: &PreparedInput,
) -> Result<Option<Box<NodeGuard>>, Refusal> {
    let Some(context) = context(prepared) else {
        return Ok(None);
    };
    NodeGuard::new(
        &node.value_type,
        quantity_profile::output_capacity(
            node,
            input,
            prepared.maximum_bytes(),
            prepared.quantity_profile(),
        )?,
        true,
        context,
    )
}
pub(super) fn output_guard(
    program: ProgramView<'_>,
    input: &PreparedInput,
    capacity: usize,
) -> Result<Option<Box<NodeGuard>>, Refusal> {
    let Some(context) = context(input) else {
        return Ok(None);
    };
    if leaf_kind(program.output_type).is_ok() {
        return Ok(None);
    }
    NodeGuard::new(program.output_type, capacity, false, context)
}
pub(super) fn input_guard(
    program: &PortableExpressionProgram,
    context: Option<&Context>,
) -> Result<Option<Box<NodeGuard>>, Refusal> {
    let Some(context) = context else {
        return Ok(None);
    };
    NodeGuard::new(
        &program.input_type,
        program.maximum_prepared_input_bytes()? as usize,
        false,
        context,
    )
}
impl PreparedPortableExpressionEvaluator {
    /// Source-law checked interpreter preparation. This does not authenticate
    /// an expression envelope/Kind, install a Host Back or authorize an offer.
    pub fn new_checked_native_laws(
        program: &PortableExpressionProgram,
        registry: &NativeLawRegistry,
        bounds: PreparedExpressionLawBounds,
    ) -> Result<Self, Refusal> {
        program
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?;
        Self::new_profile(
            program,
            conduit_core::QuantityValueProfile::Decimal,
            ArithmeticPolicy::Checked,
            Some(Context::new(registry, bounds)?),
        )
    }
    pub fn checked_native_law_capacity(&self) -> Option<PreparedExpressionLawCapacityReport> {
        let context = context(&self.input)?;
        let used = &context.0.used;
        Some(PreparedExpressionLawCapacityReport {
            guards: used[0].get(),
            canonical_buffer_capacity: used[1].get(),
            scopes: used[2].get(),
            evaluators: used[3].get(),
            contracts: used[4].get(),
            admission_step_bound: used[5].get(),
        })
    }
    /// Terminal for the entire retained expression, including all lazy branches.
    pub fn cancel_checked_native_laws(&mut self) {
        if let Some(context) = context(&self.input) {
            if context.0.terminal.get() == 0 {
                context.0.terminal.set(1);
            }
        }
    }
}

/// Stable expression Back details; existing arithmetic/constraint codes stay fixed.
pub fn native_law_failure_detail(reason: &PreparedNativeRefusal) -> u16 {
    match reason {
        PreparedNativeRefusal::Bounds => 301,
        PreparedNativeRefusal::MissingNativeMetadata => 302,
        PreparedNativeRefusal::ConflictingNativeMetadata => 303,
        PreparedNativeRefusal::InvalidProgram => 304,
        PreparedNativeRefusal::Shape => 305,
        PreparedNativeRefusal::Contract { .. } => 306,
        PreparedNativeRefusal::Invariant { .. } => 307,
        PreparedNativeRefusal::Evaluation { .. } => 308,
        PreparedNativeRefusal::State => 309,
        PreparedNativeRefusal::Cancelled => 310,
    }
}
