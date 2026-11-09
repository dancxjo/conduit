// Explicit authority origin; both paths retain the same Source-selected owners.
// No online command, member receipt or Started stamp is synthesized for offline
// continuation. Each existing issuer/group entry independently checks its origin.
use conduit_body as b;
use conduit_core as c;
use conduit_mcu_host_runtime::{
    continuation_native::ContinuationNativeContext,
    execution::{Policy, Refusal},
    execution_authority::{self, Issued, Issuer, NativeFacts, Operation},
    execution_group::{self, EntryPin},
    execution_workset::{
        CommittedLocalWorkset, PreparedLocalKernels, PreparedLocalWorkset, StartedLocalWorkset,
    },
};
#[derive(Clone, Copy)]
pub enum NativeAuthority<'context, 'scope, 'work> {
    Online(b::NativePreparationContext<'context>),
    Continuation {
        context: ContinuationNativeContext<'context>,
        scope: &'scope b::FreshContinuationScope<'work>,
    },
}
impl<'c, 's, 'w> From<b::NativePreparationContext<'c>> for NativeAuthority<'c, 's, 'w> {
    fn from(context: b::NativePreparationContext<'c>) -> Self {
        Self::Online(context)
    }
}
impl<'c, 's, 'w> NativeAuthority<'c, 's, 'w> {
    pub fn actual(self) -> &'c c::HostAdvertisement {
        match self {
            Self::Online(context) => context.actual,
            Self::Continuation { context, .. } => context.actual(),
        }
    }
    pub(crate) fn body_id(self) -> &'c b::BodyId {
        match self {
            Self::Online(context) => &context.member.body_id,
            Self::Continuation { context, .. } => context.body_id(),
        }
    }
    pub(crate) fn issue(
        self,
        issuer: &mut Issuer,
        policy: Policy<'_>,
        approved: &b::DelegatedBodyFragment<'_>,
        native: &NativeFacts<'_>,
        operation: &Operation<'_>,
        maximum_bytes: usize,
    ) -> Result<Issued, execution_authority::Refusal> {
        match self {
            Self::Online(context) => {
                issuer.issue(context, policy, approved, native, operation, maximum_bytes)
            }
            Self::Continuation { context, scope } => issuer.issue_continuation(
                context,
                policy,
                scope,
                approved,
                native,
                operation,
                maximum_bytes,
            ),
        }
    }
    #[inline(always)]
    pub(crate) fn start_into<T: PreparedLocalKernels, const F: usize, E>(
        self,
        committed: &mut CommittedLocalWorkset<'_, T, F>,
        workspace: Option<&b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>>,
        policy: Policy<'_>,
        resources: &c::ResourceAdmissionOwner,
        target: &mut Option<StartedLocalWorkset<T::Bound>>,
    ) -> Result<(), Refusal<E>> {
        match (self, workspace) {
            (Self::Online(context), Some(workspace)) => {
                committed.start_into(workspace, context, policy, resources, target)
            }
            (Self::Continuation { context, scope }, None) => {
                committed.start_continuation_into(scope, context, policy, resources, target)
            }
            _ => Err(Refusal::Scope),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare<'p, T, E, const INPUT: usize, const PORTS: usize>(
        self,
        workspace: Option<&'w b::HostCommandInput<INPUT>>,
        policy: Policy<'_>,
        pins: [EntryPin<'p>; 3],
        configurations: [&'p [c::AssignedRecordRequirement]; 3],
        profiles: [b::AssignedBodyPreparationProfile<'p>; 3],
        factory: impl FnOnce(
            [&b::DelegatedBodyFragment<'w>; 3],
            [&[c::AssignedForeRequirement]; 3],
        ) -> Result<T, E>,
    ) -> Result<PreparedLocalWorkset<'w, T, 3>, Refusal<Refusal<E>>>
    where
        T: PreparedLocalKernels,
    {
        match (self, workspace) {
            (Self::Online(context), Some(workspace)) => {
                execution_group::prepare_with_source::<T, E, INPUT, PORTS>(
                    workspace,
                    context,
                    policy,
                    pins,
                    configurations,
                    profiles,
                    factory,
                )
            }
            (Self::Continuation { context, scope }, None) => {
                execution_group::prepare_continuation_with_source::<T, E, PORTS>(
                    scope,
                    context,
                    policy,
                    pins,
                    configurations,
                    profiles,
                    factory,
                )
            }
            _ => Err(Refusal::Scope),
        }
    }
}
