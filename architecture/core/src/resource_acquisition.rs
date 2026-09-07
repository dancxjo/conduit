//! Generic attended Resource acquisition lifecycle.

use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquiredResource {
    pub identity: String,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceAcquisitionState {
    Discoverable,
    Requested { request_id: String },
    Current(AcquiredResource),
    Released { generation: u64 },
    Revoked { generation: u64 },
    Lost { generation: u64 },
    Denied,
    Cancelled,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResourceAcquisitionRefusal {
    InvalidTransition,
    StaleGeneration,
    InvalidIdentity,
}

impl ResourceAcquisitionState {
    pub fn request(&self, request_id: String) -> Result<Self, ResourceAcquisitionRefusal> {
        if request_id.is_empty() {
            return Err(ResourceAcquisitionRefusal::InvalidIdentity);
        }
        match self {
            Self::Discoverable
            | Self::Released { .. }
            | Self::Revoked { .. }
            | Self::Lost { .. }
            | Self::Denied
            | Self::Cancelled => Ok(Self::Requested { request_id }),
            _ => Err(ResourceAcquisitionRefusal::InvalidTransition),
        }
    }
    pub fn acquire(&self, resource: AcquiredResource) -> Result<Self, ResourceAcquisitionRefusal> {
        if resource.identity.is_empty() {
            return Err(ResourceAcquisitionRefusal::InvalidIdentity);
        }
        match self {
            Self::Requested { .. } => Ok(Self::Current(resource)),
            _ => Err(ResourceAcquisitionRefusal::InvalidTransition),
        }
    }
    pub fn release(&self) -> Result<Self, ResourceAcquisitionRefusal> {
        self.terminal(false, false)
    }
    pub fn revoke(&self) -> Result<Self, ResourceAcquisitionRefusal> {
        self.terminal(true, false)
    }
    pub fn lose(&self) -> Result<Self, ResourceAcquisitionRefusal> {
        self.terminal(false, true)
    }
    fn terminal(&self, revoked: bool, lost: bool) -> Result<Self, ResourceAcquisitionRefusal> {
        let Self::Current(resource) = self else {
            return Err(ResourceAcquisitionRefusal::InvalidTransition);
        };
        Ok(if revoked {
            Self::Revoked {
                generation: resource.generation,
            }
        } else if lost {
            Self::Lost {
                generation: resource.generation,
            }
        } else {
            Self::Released {
                generation: resource.generation,
            }
        })
    }
    pub fn accepts(
        &self,
        identity: &str,
        generation: u64,
    ) -> Result<(), ResourceAcquisitionRefusal> {
        match self {
            Self::Current(resource)
                if resource.identity == identity && resource.generation == generation =>
            {
                Ok(())
            }
            _ => Err(ResourceAcquisitionRefusal::StaleGeneration),
        }
    }
}
