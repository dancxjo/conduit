//! Fixed storage projection of a local verified fragment for the existing kernel.
//! This is not provider possession, complete memory admission or Body start.
use crate::lowering::{lower_verified_plan_metadata_fragment, LoweredPlanFragment};
use conduit_core::{FragmentId, VerifiedPlanMetadata};
use conduit_kernel::{
    self as k,
    scheduler::{CordSpec, NodeSpec},
};

#[derive(Debug, PartialEq, Eq)]
pub enum LocalTableRefusal {
    Scope,
    Lowering,
    Capacity,
    Unsupported,
    Kernel(k::ProtocolError),
}
/// Own one exact lowering from a Core-verified fragment. The private fields
/// preserve provenance so consumers need not recreate a second rich lowering
/// while the first occupies constrained preparation memory.
pub struct VerifiedLocalLowering<'a> {
    plan: &'a VerifiedPlanMetadata,
    lowered: LoweredPlanFragment,
}
impl<'a> VerifiedLocalLowering<'a> {
    pub fn prepare(
        plan: &'a VerifiedPlanMetadata,
        fragment: &FragmentId,
    ) -> Result<Self, LocalTableRefusal> {
        if plan.fragment(fragment).is_none() {
            return Err(LocalTableRefusal::Scope);
        }
        let lowered = lower_verified_plan_metadata_fragment(plan, fragment)
            .map_err(|_| LocalTableRefusal::Lowering)?;
        Ok(Self { plan, lowered })
    }
    pub fn lowered(&self) -> &LoweredPlanFragment {
        &self.lowered
    }
    pub fn preparation_scope(&self) -> (&VerifiedPlanMetadata, &LoweredPlanFragment) {
        (self.plan, &self.lowered)
    }
}
/// No assigned wire identity is fabricated. Exact Plan and lowering stay borrowed
/// throughout preparation, retaining resource, sign and terminal obligations.
/// Native resources need separately installed Host Call possession before Play.
pub struct VerifiedLocalKernelTables<
    'a,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
> {
    plan: &'a VerifiedPlanMetadata,
    lowered: &'a LoweredPlanFragment,
    pub nodes: [NodeSpec<P>; N],
    pub cords: [CordSpec; C],
    pub routes: k::FixedRoutes<R, T>,
    pub calls: k::FixedHostCallBindings<H>,
    startup_order: [k::NodeId; N],
}
impl<
        'a,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    > VerifiedLocalKernelTables<'a, N, C, P, R, T, H>
{
    /// Reproduce ordinary lowering before filling finite destinations. Unsupported
    /// retained execution contracts refuse instead of disappearing from tables.
    pub fn prepare(
        plan: &'a VerifiedPlanMetadata,
        fragment: &FragmentId,
        lowered: &'a LoweredPlanFragment,
    ) -> Result<Self, LocalTableRefusal> {
        use LocalTableRefusal as E;
        if plan.fragment(fragment).is_none() {
            return Err(E::Scope);
        }
        if lower_verified_plan_metadata_fragment(plan, fragment).map_err(|_| E::Lowering)?
            != *lowered
        {
            return Err(E::Lowering);
        }
        Self::prepare_checked(plan, fragment, lowered)
    }
    /// Project the already checked lowering without making a duplicate. The
    /// borrow keeps its exact verified Plan scope alive through table use.
    pub fn from_verified_lowering(
        verified: &'a VerifiedLocalLowering<'a>,
    ) -> Result<Self, LocalTableRefusal> {
        Self::prepare_checked(
            verified.plan,
            &verified.lowered.identity.fragment_id,
            &verified.lowered,
        )
    }
    fn prepare_checked(
        plan: &'a VerifiedPlanMetadata,
        fragment: &FragmentId,
        lowered: &'a LoweredPlanFragment,
    ) -> Result<Self, LocalTableRefusal> {
        use LocalTableRefusal as E;
        let view = plan.fragment(fragment).ok_or(E::Scope)?;
        let metadata = view.lowering_metadata();
        if N == 0
            || P == 0
            || N > usize::from(u16::MAX)
            || P > usize::from(u16::MAX)
            || lowered.nodes.len() != N
            || lowered.node_specs.len() != N
            || lowered.cords.len() != C
            || lowered.routes.len() > R
            || lowered.host_calls.len() > H
        {
            return Err(E::Capacity);
        }
        if !lowered.remote_endpoints.is_empty()
            || !lowered.fore_ports.is_empty()
            || !lowered.states.is_empty()
            || !lowered.fusions.is_empty()
            || !lowered.resource_cords.is_empty()
            || !lowered.shared_pools.is_empty()
            || lowered
                .nodes
                .iter()
                .any(|node| !node.terminal_transductions.is_empty())
        {
            return Err(E::Unsupported);
        }
        if metadata.execution_regions.len() > 1
            || metadata.execution_regions.iter().any(|region| {
                region.lane_count != 1
                    || region.scheduling
                        != conduit_core::ExecutionScheduling::CooperativeBoundedStep
                    || region.admitted_placements.len() != N
            })
        {
            return Err(E::Unsupported);
        }
        // This projection retains ordinary native attachments through the borrowed
        // lowering. General resource execution needs a separate installed owner.
        if lowered.resources.iter().any(|resource| {
            !lowered
                .host_calls
                .iter()
                .any(|call| call.node == resource.node)
        }) {
            return Err(E::Unsupported);
        }
        let mut nodes = [NodeSpec {
            input_cords: [None; P],
            maximum_step_fuel: 0,
        }; N];
        for ((output, spec), node) in nodes
            .iter_mut()
            .zip(&lowered.node_specs)
            .zip(&lowered.nodes)
        {
            if node.inputs.len() > P
                || node.outputs.len() > P
                || spec.input_cords.iter().skip(P).any(Option::is_some)
            {
                return Err(E::Capacity);
            }
            output.maximum_step_fuel = spec.maximum_step_fuel;
            for (output, input) in output.input_cords.iter_mut().zip(spec.input_cords) {
                *output = input;
            }
        }
        let mut cords = [CordSpec::inactive(); C];
        for (index, (output, cord)) in cords.iter_mut().zip(&lowered.cords).enumerate() {
            if usize::from(cord.spec.cord.0) != index {
                return Err(E::Scope);
            }
            for endpoint in [cord.spec.source, cord.spec.sink] {
                match endpoint {
                    k::CordEndpoint::Local { node, port }
                        if usize::from(node.0) < N && usize::from(port.0) < P => {}
                    _ => return Err(E::Unsupported),
                }
            }
            *output = cord.spec;
        }
        let mut routes = k::FixedRoutes::new(P as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(E::Kernel)?;
        }
        routes.seal().map_err(E::Kernel)?;
        let mut calls = k::FixedHostCallBindings::new(1);
        for call in &lowered.host_calls {
            if call.call != k::HostCallId(0) || call.maximum_in_flight != 1 {
                return Err(E::Unsupported);
            }
            calls.install(call.node, call.binding).map_err(E::Kernel)?;
        }
        calls.seal().map_err(E::Kernel)?;
        let mut startup_order = [k::NodeId(u16::MAX); N];
        if metadata.startup_order.len() != N {
            return Err(E::Scope);
        }
        for (output, placement) in startup_order.iter_mut().zip(&metadata.startup_order) {
            *output = lowered
                .identity
                .node_for_placement(placement)
                .ok_or(E::Scope)?;
        }
        Ok(Self {
            plan,
            lowered,
            nodes,
            cords,
            routes,
            calls,
            startup_order,
        })
    }
    pub fn preparation_scope(&self) -> (&'a VerifiedPlanMetadata, &'a LoweredPlanFragment) {
        (self.plan, self.lowered)
    }
    pub fn startup_order(&self) -> &[k::NodeId; N] {
        &self.startup_order
    }
}
