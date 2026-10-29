#![no_std]
//! Selected network Host Calls. No radio initialization or scheduling policy.
extern crate alloc;
mod contract;
pub mod credential_installation;
mod credentials;
pub use credentials::{
    CredentialCallRefusal, CredentialSource, CredentialSourceRefusal, NetworkCredentialsHostCall,
    PreparedCredentialContract, CREDENTIAL_RESOURCE_CLASS,
};
mod control_kernel;
pub use control_kernel::{
    CheckedNetworkLowering, ControlKernelRefusal, NetworkControlDirectTables,
    NetworkControlFlatDirectTables, NetworkControlKernel, NetworkControlRoles,
    NetworkKernelAllocationFailure, NetworkKernelAllocationPhase, PreparedNetworkKernel,
    PreparedNetworkKernelParts, PreparedNetworkKernelStorage, CONTROL_KERNEL_BYTES, CONTROL_PORTS,
    CONTROL_SIGN_BACKING_BYTES,
};
mod control_back;
pub use control_back::NetworkControlBack;
pub mod installation;
mod lease;
mod owner;
pub mod recovery_evidence;
pub use contract::{AttachmentRecord, NetworkJoinProvider, PreparedJoinContract};
pub use owner::{AssignedJoinSelection, JoinCallRefusal, NetworkJoinHostCall};

mod pair;
pub use pair::*;
