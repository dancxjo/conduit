//! Shared kernel lifecycle for one exact pure expression operation.

/// A pure expression uses the same finite Host Call lifecycle as machine bases.
pub type PureExpressionBack = conduit_kernel::scheduler::HostCallBack;
