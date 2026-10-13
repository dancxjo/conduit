//! Unwired private transaction ingress ownership. Type admission remains in
//! PreparedParserSourceFlow; no parser policy or graph legality is implemented.
use crate::parser_session_runtime::{
    ParserSourceExecutor, ParserSourceFlowRefusal, PreparedParserSourceFlow,
};
use crate::parser_session_transaction::SessionTransactionStage;
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use conduit_plot::rust_binding::NativeRustBinding;
use core::cell::{Cell, RefCell};

trait CancelIngress {
    fn cancel(&self);
}
struct OwnedFlow<I, O, E> {
    flow: RefCell<PreparedParserSourceFlow<I, O, E>>,
}
impl<I: NativeRustBinding, O: NativeRustBinding, E: ParserSourceExecutor> CancelIngress
    for OwnedFlow<I, O, E>
{
    fn cancel(&self) {
        self.flow.borrow_mut().cancel();
    }
}
struct CancelHandle<I, O, E>(Rc<OwnedFlow<I, O, E>>);
impl<I: NativeRustBinding, O: NativeRustBinding, E: ParserSourceExecutor> CancelIngress
    for CancelHandle<I, O, E>
{
    fn cancel(&self) {
        self.0.cancel();
    }
}
/// All registered ingresses close together after ambiguous or unpublished
/// consumption. Handles never expose their executor or unguarded Flow.
pub(crate) struct TransactionIngressRegistry {
    closed: Cell<bool>,
    sealed: Cell<bool>,
    limit: u16,
    ports: RefCell<Vec<Box<dyn CancelIngress>>>,
}
#[derive(Debug)]
pub(crate) enum IngressRegistryRefusal {
    Limits,
    Pressure,
    Allocation,
    Cancelled,
    Sealed,
}
impl TransactionIngressRegistry {
    pub(crate) fn new(limit: u16) -> Result<Rc<Self>, IngressRegistryRefusal> {
        if limit == 0 {
            return Err(IngressRegistryRefusal::Limits);
        }
        let mut ports = Vec::new();
        ports
            .try_reserve_exact(usize::from(limit))
            .map_err(|_| IngressRegistryRefusal::Allocation)?;
        Ok(Rc::new(Self {
            closed: Cell::new(false),
            sealed: Cell::new(false),
            limit,
            ports: RefCell::new(ports),
        }))
    }
    pub(crate) fn seal(&self) {
        self.sealed.set(true);
    }
    pub(crate) fn closed(&self) -> bool {
        self.closed.get()
    }
    pub(crate) fn close(&self) {
        self.closed.set(true);
        for port in self.ports.borrow().iter() {
            port.cancel();
        }
    }
    pub(crate) fn register<
        I: NativeRustBinding + 'static,
        O: NativeRustBinding + 'static,
        E: ParserSourceExecutor + 'static,
    >(
        self: &Rc<Self>,
        flow: PreparedParserSourceFlow<I, O, E>,
    ) -> Result<GuardedSessionIngress<I, O, E>, IngressRegistryRefusal> {
        if self.closed() {
            return Err(IngressRegistryRefusal::Cancelled);
        }
        if self.sealed.get() {
            return Err(IngressRegistryRefusal::Sealed);
        }
        let mut ports = self.ports.borrow_mut();
        if ports.len() == usize::from(self.limit) {
            return Err(IngressRegistryRefusal::Pressure);
        }
        let owned = Rc::new(OwnedFlow {
            flow: RefCell::new(flow),
        });
        ports.push(Box::new(CancelHandle(owned.clone())));
        Ok(GuardedSessionIngress {
            owned,
            registry: self.clone(),
        })
    }
}
pub(crate) enum GuardedIngressRefusal<E> {
    Stage(crate::parser_session_transaction::SessionTransactionRefusal),
    Flow(ParserSourceFlowRefusal<E>),
}
pub(crate) struct GuardedSessionIngress<I, O, E> {
    owned: Rc<OwnedFlow<I, O, E>>,
    registry: Rc<TransactionIngressRegistry>,
}
impl<I: NativeRustBinding, O: NativeRustBinding, E: ParserSourceExecutor>
    GuardedSessionIngress<I, O, E>
{
    pub(crate) fn is_cancelled(&self) -> bool {
        self.registry.closed() || self.owned.flow.borrow().is_cancelled()
    }
    pub(crate) fn transact(
        &mut self,
        stage: &mut SessionTransactionStage<'_>,
        input: I,
    ) -> Result<O, GuardedIngressRefusal<E::Error>> {
        stage
            .before_ingress(&self.registry)
            .map_err(GuardedIngressRefusal::Stage)?;
        // Arm abandonment before entering a target: unwinding must also close
        // sibling ingresses even when no normal response reaches record_ingress.
        stage.record_ingress(true, false);
        // Release the mutable executor borrow before closing the whole registry.
        let (result, ambiguous) = {
            let mut flow = self.owned.flow.borrow_mut();
            let result = flow.transact(input);
            let ambiguous = result.is_ok() || flow.is_poisoned();
            (result, ambiguous)
        };
        stage.record_ingress(ambiguous, result.is_err());
        result.map_err(GuardedIngressRefusal::Flow)
    }
}

/// The Session uses these ports for graph-changing executions. Unlike a raw
/// typed Flow, every result retains the original I/O and fixed Source witness.
struct OwnedVerifiedPort<I, O, E> {
    port: RefCell<crate::parser_session_execution::PreparedParserSessionPort<I, O, E>>,
}
impl<I: NativeRustBinding + Clone, O: NativeRustBinding + Clone, E: ParserSourceExecutor>
    CancelIngress for Rc<OwnedVerifiedPort<I, O, E>>
{
    fn cancel(&self) {
        self.port.borrow_mut().cancel();
    }
}
pub(crate) struct GuardedVerifiedIngress<I, O, E> {
    owned: Rc<OwnedVerifiedPort<I, O, E>>,
    registry: Rc<TransactionIngressRegistry>,
}
impl TransactionIngressRegistry {
    pub(crate) fn register_verified<
        I: NativeRustBinding + Clone + 'static,
        O: NativeRustBinding + Clone + 'static,
        E: ParserSourceExecutor + 'static,
    >(
        self: &Rc<Self>,
        port: crate::parser_session_execution::PreparedParserSessionPort<I, O, E>,
    ) -> Result<GuardedVerifiedIngress<I, O, E>, IngressRegistryRefusal> {
        if self.closed() {
            return Err(IngressRegistryRefusal::Cancelled);
        }
        if self.sealed.get() {
            return Err(IngressRegistryRefusal::Sealed);
        }
        let mut ports = self.ports.borrow_mut();
        if ports.len() == usize::from(self.limit) {
            return Err(IngressRegistryRefusal::Pressure);
        }
        let owned = Rc::new(OwnedVerifiedPort {
            port: RefCell::new(port),
        });
        ports.push(Box::new(owned.clone()));
        Ok(GuardedVerifiedIngress {
            owned,
            registry: self.clone(),
        })
    }
}
impl<I: NativeRustBinding + Clone, O: NativeRustBinding + Clone, E: ParserSourceExecutor>
    GuardedVerifiedIngress<I, O, E>
{
    pub(crate) fn is_cancelled(&self) -> bool {
        self.registry.closed() || self.owned.port.borrow().is_cancelled()
    }
    pub(crate) fn next_ordinal(&self) -> u64 {
        self.owned.port.borrow().next_ordinal()
    }
    pub(crate) fn transact(
        &mut self,
        stage: &mut SessionTransactionStage<'_>,
        input: I,
    ) -> Result<
        crate::parser_session_execution::ParserSessionExecution<I, O>,
        GuardedVerifiedIngressRefusal<E::Error>,
    > {
        stage
            .before_ingress(&self.registry)
            .map_err(GuardedVerifiedIngressRefusal::Stage)?;
        stage.record_ingress(true, false);
        let result = { self.owned.port.borrow_mut().execute(input) };
        // Conservative cancellation even when a returned encoding refusal did
        // not consume: no ambiguous target call can leave a sibling port open.
        stage.record_ingress(true, result.is_err());
        result.map_err(GuardedVerifiedIngressRefusal::Execution)
    }
}
pub(crate) enum GuardedVerifiedIngressRefusal<E> {
    Stage(crate::parser_session_transaction::SessionTransactionRefusal),
    Execution(crate::parser_session_execution::ParserSessionExecutionRefusal<E>),
}

#[path = "parser_canonical_transaction_ingress.rs"]
mod canonical;
pub(crate) use canonical::{CanonicalRegistryRefusal, GuardedCanonicalIngress};
